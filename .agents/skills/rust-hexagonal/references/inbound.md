# Inbound Adapter Reference

Inbound adapters translate external requests into calls to domain use case functions. They are
generic over domain port traits and never import concrete outbound adapter types.

The `inbound` crate can expose one aggregate `AppState<...>` type or accept multiple typed
framework app-data values. Both are valid as long as handlers depend on traits, not concrete
adapters.

---

## actix-web (preferred)

### `inbound/src/state.rs` (aggregate state shape)

```rust
// inbound/src/state.rs
use domain::ports::repository::OrderRepository;

// Combines all outbound traits needed by this adapter.
// As bounded contexts grow, add new repository supertrait variants
// (e.g. CustomerAppRepository) rather than expanding this one.
pub trait AppRepository: OrderRepository + Send + Sync + 'static {}

// Blanket impl: any type that satisfies the bounds is automatically an AppRepository.
// This means SeaOrmOrderRepository (outbound) never needs to name this inbound trait.
impl<T: OrderRepository + Send + Sync + 'static> AppRepository for T {}

// Generic over R so the adapter has zero knowledge of concrete types.
// web::Data<AppState<R>> (actix) or Arc<AppState<R>> (axum) provides
// the shared-ownership wrapper — no Arc needed in this struct.
pub struct AppState<R: AppRepository> {
    pub repo: R,
}
```

> **When the application also has non-repository ports** (token, cache, email), extend the
> type params:
>
> ```rust
> // Two-generic variant — use when a TokenService is a separate domain port
> pub struct AppState<TS: TokenService, R: AppRepository> {
>     pub token_service: TS,
>     pub repository: R,
> }
> ```
>
> Handlers then receive `web::Data<AppState<TS, R>>` and the JWT extractor reads from
> `state.token_service`. See the **JWT extractor** section below.

### `inbound/src/lib.rs`

```rust
pub mod http;
pub mod state;
```

### Handler (aggregate state)

```rust
use actix_web::{web, HttpResponse, Responder, post, get};
use domain::{error::DomainError, use_cases::orders};
use uuid::Uuid;
use crate::state::{AppRepository, AppState};
use super::dto::OrderResponse;

// Handlers are generic over R: AppRepository.
// web::Data<AppState<R>> is extracted by actix-web from its type map;
// the concrete R is only named in app/src/main.rs.

#[get("/{id}")]
pub async fn get_order<R: AppRepository>(
    path: web::Path<Uuid>,
    state: web::Data<AppState<R>>,
) -> Result<impl Responder, ApiError> {
    // `?` converts DomainError via From<DomainError> for ApiError
    let order = orders::get_order(&state.repo, *path).await?;
    Ok(HttpResponse::Ok().json(OrderResponse::from(order)))
}

#[post("/{id}/confirm")]
pub async fn confirm_order<R: AppRepository>(
    path: web::Path<Uuid>,
    state: web::Data<AppState<R>>,
) -> Result<impl Responder, ApiError> {
    let order = orders::confirm_order(&state.repo, *path).await?;
    Ok(HttpResponse::Ok().json(OrderResponse::from(order)))
}

#[get("")]
pub async fn list_orders<R: AppRepository>(
    state: web::Data<AppState<R>>,
) -> Result<impl Responder, ApiError> {
    let orders = orders::list_orders(&state.repo).await?;
    Ok(HttpResponse::Ok().json(orders.map(OrderResponse::from).collect::<Vec<_>>()))
}
```

### Router Configuration

The version prefix is declared **once** at the router level; handlers never reference it.

```rust
use actix_web::web;

// Generic over R so the concrete type is only named in app/src/main.rs.
// Called as: .configure(order_routes::<SeaOrmOrderRepository>)
pub fn order_routes<R: AppRepository + 'static>(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(
                web::scope("/orders")
                    .service(get_order::<R>)
                    .service(confirm_order::<R>)
                    .service(list_orders::<R>),
            ),
    );
}
```

The concrete type for `R` is only filled in at `app/main.rs`.

### Alternative: multiple typed app-data values

Instead of one `AppState<TS, R, ...>`, handlers can receive separate typed values:

```rust
use actix_web::web;
use domain::ports::{repository::OrderRepository, service::TokenService};

pub async fn list_orders<TS, R>(
    token_service: web::Data<TS>,
    repository: web::Data<R>,
) -> Result<impl Responder, ApiError>
where
    TS: TokenService,
    R: OrderRepository,
{
    let _claims = token_service.verify_token("...")?;
    let orders = orders::list_orders(repository.get_ref()).await?;
    Ok(HttpResponse::Ok().json(orders.collect::<Vec<_>>()))
}
```

Choose this shape when handlers use many independent adapters and an aggregate state would become
too broad.

### DTOs

```rust
// inbound/src/http/dto.rs
use domain::model::order::{Order, OrderStatus};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub status: String,
}

impl From<Order> for OrderResponse {
    fn from(o: Order) -> Self {
        OrderResponse {
            id: o.id.0.to_string(),
            status: format!("{:?}", o.status),
        }
    }
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub customer_id: String,
}
```

DTOs must **never** expose domain types directly in their fields. Map at the boundary.

> **Preferred layout: co-locate with handlers** (per-bounded-context)
>
> Instead of a top-level `dto.rs`, place request and response types alongside the handlers
> that use them. This keeps each bounded context self-contained:
>
> ```
> inbound/src/http/
> └── orders/
>     ├── mod.rs          # configure fn
>     ├── handler.rs      # handler fns
>     ├── payload.rs      # Deserialize request structs + Validate
>     └── response.rs     # Serialize response structs + From<DomainType>
> ```

### `inbound/Cargo.toml`

```toml
[package]
name = "inbound"
version = "0.1.0"
edition = "2021"

[dependencies]
domain.workspace = true
serde.workspace  = true
uuid.workspace   = true
tokio.workspace  = true
# layer-specific — not in workspace
actix-web = "4"
```

---

## axum (alternative)

### Handler

```rust
// inbound/src/http/orders.rs  (axum variant)
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use domain::{error::DomainError, use_cases::orders};
use uuid::Uuid;
use crate::state::{AppRepository, AppState};
use super::dto::OrderResponse;

pub async fn get_order<R: AppRepository>(
    Path(id): Path<Uuid>,
    State(state): State<Arc<AppState<R>>>,
) -> impl IntoResponse {
    match orders::get_order(&state.repo, id).await {
        Ok(order)                          => (StatusCode::OK, Json(OrderResponse::from(order))).into_response(),
        Err(DomainError::OrderNotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(_)                             => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
```

### Router

The version prefix is declared **once** via `.nest`; handlers never reference it.

```rust
// inbound/src/http/router.rs  (axum variant)
use axum::{routing::get, Router};
use std::sync::Arc;
use crate::state::{AppRepository, AppState};
use super::orders::get_order;

// Arc<AppState<R>> is always Clone (Arc is Clone regardless of R),
// so no Clone bound is needed on R.
pub fn order_router<R: AppRepository>(
    state: Arc<AppState<R>>,
) -> Router {
    Router::new()
        .nest("/api/v1", Router::new()
            .nest("/orders", Router::new()
                .route("/{id}", get(get_order::<R>)),
            ),
        )
        .with_state(state)
}
```

### `inbound/Cargo.toml` (axum variant)

```toml
[dependencies]
domain.workspace = true
serde.workspace  = true
uuid.workspace   = true
tokio.workspace  = true
# layer-specific — not in workspace
axum = "0.8"
```

---

## HTTP Best Practices

> Load the **`http-actix-axum` skill** for the full reference. Summary of key points:
>
> - Scope all routes under `/api/v1/` in `router.rs`; handlers never reference the version prefix
> - POST creates → **201 Created** + `Location` header; DELETE → **204**; async ops → **202**
> - All errors return `Content-Type: application/problem+json` (RFC 9457) — see `errors.rs` below
> - Validate DTOs with `validator` crate; return 400 problem detail on failure

### Error handling skeleton (`inbound/src/http/errors.rs`)

```rust
use actix_web::{HttpResponse, ResponseError};
use domain::error::DomainError;
use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub struct ProblemDetail {
    #[serde(rename = "type")] pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
}

#[derive(Debug)]
pub struct ApiError(ProblemDetail);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0.title) }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::from_u16(self.0.status).unwrap()
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .content_type("application/problem+json")
            .json(&self.0)
    }
}

impl From<DomainError> for ApiError {
    fn from(e: DomainError) -> Self {
        let (status, title, detail) = match &e {
            DomainError::OrderNotFound(id) => (404, "Order Not Found", format!("No order {id}")),
            DomainError::InvalidStatusTransition => (422, "Invalid Transition", "...".into()),
            DomainError::Infrastructure(_) => (500, "Internal Server Error", "...".into()),
        };
        ApiError(ProblemDetail {
            problem_type: format!("https://example.com/errors/{}", title.to_lowercase().replace(' ', "-")),
            title: title.into(), status, detail,
        })
    }
}
```

Handlers return `Result<impl Responder, ApiError>`; `?` converts `DomainError` via `From`.

### JWT extractor (`FromRequest`)

When JWT authentication is required, implement a typed `FromRequest` extractor that validates
the token against the `TokenService` port. The extractor can read either aggregate state or
standalone app-data values.

```rust
// inbound/src/http/middleware/auth.rs
use actix_web::{web, FromRequest, HttpRequest};
use actix_web::dev::Payload;
use actix_web::http::header;
use domain::{model::TokenClaims, ports::{AppRepository, TokenService}};
use std::{future::ready, marker::PhantomData};
use crate::config::AppState;
use crate::http::error::ApiError;

/// Typed extractor — parameterised so it can access `AppState<TS, R>` from app_data.
/// Handlers receive `claims: JwtClaims<TS, R>` and access the inner value as `claims.0`.
#[derive(Debug)]
pub struct JwtClaims<TS: TokenService, R: AppRepository>(
    pub TokenClaims,
    PhantomData<(TS, R)>,  // carries type params without storing values
);

impl<TS: TokenService + 'static, R: AppRepository + 'static> FromRequest
    for JwtClaims<TS, R>
{
    type Error = actix_web::Error;
    type Future = std::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let state = req.app_data::<web::Data<AppState<TS, R>>>()
            .expect("AppState not registered");
        let auth_header = match req.headers().get(header::AUTHORIZATION) {
            Some(v) => v.to_str().unwrap_or_default(),
            None    => return ready(Err(actix_web::error::ErrorUnauthorized("missing token"))),
        };
        if !auth_header.starts_with("Bearer ") {
            return ready(Err(actix_web::error::ErrorUnauthorized("invalid scheme")));
        }
        let token = &auth_header[7..];
        match state.token_service.verify_token(token) {
            Ok(claims) => ready(Ok(JwtClaims(claims, PhantomData))),
            Err(_)     => ready(Err(actix_web::error::ErrorUnauthorized(
                ApiError::from(domain::error::DomainError::InvalidToken),
            ))),
        }
    }
}
```

Usage in a protected handler:

```rust
pub async fn get_my_orders<TS: TokenService, R: AppRepository>(
    state:  web::Data<AppState<TS, R>>,
    claims: JwtClaims<TS, R>,           // 401 returned automatically if missing/invalid
) -> Result<impl Responder, ApiError> {
    let orders = orders::list_for_user(&state.repository, claims.0.sub).await?;
    Ok(HttpResponse::Ok().json(orders.map(OrderResponse::from).collect::<Vec<_>>()))
}
```

> `TokenService::verify_token` is **synchronous** — `from_request` returns `Ready<…>`,
> requiring no async machinery. Only use `async` extractors when IO is needed.

---

### Module layout

```
