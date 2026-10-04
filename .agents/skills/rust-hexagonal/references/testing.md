# Testing Patterns — Rust Hexagonal Architecture

## Layer-by-Layer Strategy

### 1. Domain Model Tests (Pure Unit)

No async, no mocks. Lives in `domain/src/model/`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_order_is_pending() {
        let order = Order::fixture();
        assert_eq!(order.status, OrderStatus::Pending);
    }
}
```

---

### 2. Use Case Tests (free functions + mock repo)

Use cases are `async fn`s that take `repo: &R`. Pass a mock or fake directly —
no struct, no `Arc<dyn ...>`.

> **mockall limitation:** `#[cfg_attr(test, mockall::automock)]` **cannot** be applied to a
> trait method that returns `impl Trait` in return position (RPIT), e.g.
> `async fn find_all() -> Result<impl Iterator<Item = Order>, DomainError>`.
>
> **Rule: only `automock` traits with concrete return types.** For RPIT traits, write a
> hand-written fake struct:
>
> ```rust
> #[cfg(test)]
> pub struct FakeOrderRepository { pub orders: Vec<Order> }
>
> #[cfg(test)]
> impl OrderRepository for FakeOrderRepository {
>     async fn find_by_id(&self, id: Uuid) -> Result<Order, DomainError> {
>         self.orders.iter().find(|o| o.id.0 == id).cloned()
>             .ok_or(DomainError::OrderNotFound(id))
>     }
>     async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError> {
>         Ok(self.orders.clone().into_iter())
>     }
>     async fn save(&self, _o: &Order) -> Result<(), DomainError> { Ok(()) }
> }
> ```
>
> For **sync** traits without RPIT (e.g. a `TokenService`), `automock` works normally:
> ```rust
> #[cfg_attr(test, mockall::automock)]
> pub trait TokenService: Send + Sync + 'static {
>     fn generate_token(&self, user_id: Uuid) -> Result<String, DomainError>;
> }
> // MockTokenService is auto-generated — use it as usual in tests
> ```

`#[cfg_attr(test, mockall::automock)]` on the outbound port generates `MockOrderRepository`.
Requires `mockall ≥ 0.12` for native async fn support.

```toml
# domain/Cargo.toml
[dev-dependencies]
mockall = "0.12"
tokio   = { workspace = true, features = ["macros", "rt"] }
```

```rust
// domain/src/use_cases/orders.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::outbound::MockOrderRepository;
    use mockall::predicate::eq;
    use uuid::Uuid;

    #[tokio::test]
    async fn get_order_returns_order() {
        let id   = Uuid::new_v4();
        let mut mock = MockOrderRepository::new();
        mock.expect_find_by_id()
            .with(eq(id))
            .times(1)
            .returning(|_| Ok(Order::fixture()));

        let result = get_order(&mock, id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn confirm_order_rejects_non_pending() {
        let id = Uuid::new_v4();
        let mut mock = MockOrderRepository::new();
        mock.expect_find_by_id()
            .returning(|_| Ok(Order { status: OrderStatus::Confirmed, ..Order::fixture() }));

        let err = confirm_order(&mock, id).await.unwrap_err();
        assert!(matches!(err, DomainError::InvalidStatusTransition));
    }
}
```

> **`find_all` / `impl Iterator`:** `.returning(|_| Ok(vec![Order::fixture()].into_iter()))`

---

### 3. Outbound Adapter Tests (SeaORM MockDatabase)

Unit-level — no real DB.

```rust
// outbound/src/db/order_repository.rs
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use uuid::Uuid;

    #[tokio::test]
    async fn find_by_id_maps_row_to_domain() {
        let id = Uuid::new_v4();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![entities::order::Model {
                id,
                customer_id: Uuid::new_v4(),
                status: "Pending".into(),
            }]])
            .into_connection();

        let result = SeaOrmOrderRepository::new(db).find_by_id(id).await.unwrap();
        assert_eq!(result.id.0, id);
    }
}
```

For real-DB integration tests, use `testcontainers`:

```toml
# outbound/Cargo.toml [dev-dependencies]
testcontainers = "0.15"
testcontainers-modules = { version = "0.3", features = ["postgres"] }
```

> See the **`sea-orm` skill** for full MockDatabase and testcontainers patterns.

---

### 4. Inbound Adapter Tests (actix-web)

Handlers are generic over `R: AppRepository`. Provide a `MockOrderRepository` wrapped in
`AppState` — no mock use case needed.

```rust
// inbound/src/http/orders.rs
#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};
    use domain::ports::outbound::MockOrderRepository;
    use crate::state::AppState;
    use crate::http::router::order_routes;

    fn mock_state(mock: MockOrderRepository) -> web::Data<AppState<MockOrderRepository>> {
        web::Data::new(AppState { repo: mock })
    }

    #[actix_web::test]
    async fn get_order_returns_200() {
        let mut mock = MockOrderRepository::new();
        mock.expect_find_by_id().returning(|_| Ok(Order::fixture()));

        let app = test::init_service(
            App::new()
                .app_data(mock_state(mock))
                .configure(order_routes::<MockOrderRepository>),
        ).await;

        let req  = test::TestRequest::get()
            .uri("/api/v1/orders/00000000-0000-0000-0000-000000000001")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn get_order_returns_problem_detail_on_404() {
        use domain::error::DomainError;
        use uuid::Uuid;
        let id = Uuid::new_v4();

        let mut mock = MockOrderRepository::new();
        mock.expect_find_by_id().returning(move |id| Err(DomainError::OrderNotFound(id)));

        let app = test::init_service(
            App::new()
                .app_data(mock_state(mock))
                .configure(order_routes::<MockOrderRepository>),
        ).await;

        let req  = test::TestRequest::get()
            .uri(&format!("/api/v1/orders/{id}"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/problem+json"
        );
    }
}
```

> See the **`http-actix-axum` skill** for additional handler test patterns including CORS and auth.

### 4b. Inbound Adapter Tests (axum)

```rust
#[tokio::test]
async fn get_order_returns_200() {
    use tower::ServiceExt;
    use axum::{body::Body, http::Request};
    use std::sync::Arc;
    use domain::ports::outbound::MockOrderRepository;

    let mut mock = MockOrderRepository::new();
    mock.expect_find_by_id().returning(|_| Ok(Order::fixture()));

    let state = Arc::new(AppState { repo: mock });
    let app   = order_router(state);
    let req   = Request::builder()
        .uri("/api/v1/orders/00000000-0000-0000-0000-000000000001")
        .body(Body::empty()).unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), 200);
}
```

---

## Test Fixture Pattern

```rust
// domain/src/model/order.rs
#[cfg(test)]
impl Order {
    pub fn fixture() -> Self {
        Order {
            id:          OrderId(uuid::Uuid::new_v4()),
            customer_id: CustomerId(uuid::Uuid::new_v4()),
            status:      OrderStatus::Pending,
        }
    }
}
```

---

## Summary Table

| Layer | Test style | Key tool |
|-------|-----------|----------|
| Domain model | sync `#[test]` | none |
| Use case functions | `#[tokio::test]` + mock repo | `mockall` |
| Outbound adapter | `#[tokio::test]` + MockDatabase | `sea_orm::MockDatabase` |
| Inbound adapter | `#[actix_web::test]` + mock repo | `actix_web::test` |
