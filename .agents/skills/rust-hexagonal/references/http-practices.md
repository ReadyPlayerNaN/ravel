# HTTP Best Practices Reference

---

## 1. API Versioning

Scope all routes under `/api/v1` at the router level. Handlers are version-agnostic.

```rust
// inbound/src/http/router.rs
pub fn configure<R: AppRepository>(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(
                web::scope("/orders")
                    .route("",              web::get().to(list_orders::<R>))
                    .route("",              web::post().to(create_order::<R>))
                    .route("/{id}",         web::get().to(get_order::<R>))
                    .route("/{id}/confirm", web::post().to(confirm_order::<R>)),
            ),
    );
}
```

---

## 2. HTTP Semantics

| Operation | Method | Success code | Notes |
|---|---|---|---|
| Fetch one | GET | 200 OK | |
| Fetch list | GET | 200 OK | |
| Create | POST | 201 Created + `Location` | |
| Full replace | PUT | 200 OK | |
| Partial update | PATCH | 200 OK | |
| Delete | DELETE | 204 No Content | |
| Async | POST | 202 Accepted | |

```rust
// create returns 201 + Location
Ok(order) => HttpResponse::Created()
    .insert_header(("Location", format!("/api/v1/orders/{}", order.id.0)))
    .json(OrderResponse::from(order)),
```

---

## 3. Problem Details (RFC 9457)

All error responses use `Content-Type: application/problem+json`.

### `inbound/src/http/errors.rs`

```rust
use actix_web::{HttpResponse, ResponseError};
use domain::error::DomainError;
use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub struct ProblemDetail {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

#[derive(Debug)]
pub struct ApiError(pub ProblemDetail);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.title)
    }
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
            DomainError::OrderNotFound(id) =>
                (404, "Order Not Found", format!("No order with id {id}")),
            DomainError::InvalidStatusTransition =>
                (422, "Invalid Status Transition", "Transition not allowed from current state".into()),
            DomainError::Infrastructure(_) =>
                (500, "Internal Server Error", "An unexpected error occurred".into()),
        };
        ApiError(ProblemDetail {
            problem_type: format!("https://example.com/errors/{}", title.to_lowercase().replace(' ', "-")),
            title: title.into(),
            status,
            detail,
            instance: None,
        })
    }
}
```

Handlers return `Result<impl Responder, ApiError>`; `?` converts `DomainError` automatically:

```rust
pub async fn get_order<R: AppRepository>(
    path: web::Path<Uuid>,
    state: web::Data<AppState<R>>,
) -> Result<impl Responder, ApiError> {
    let order = orders::get_order(&state.repo, *path).await?;
    Ok(HttpResponse::Ok().json(OrderResponse::from(order)))
}
```

---

## 4. Request Validation

```rust
// inbound/src/http/dto.rs
use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrderRequest {
    #[validate(length(min = 1))]
    pub customer_id: String,
    #[validate(range(min = 1))]
    pub quantity: u32,
}
```

Validate in the handler and map errors to a 400 problem detail:

```rust
body.validate().map_err(|e| ApiError(ProblemDetail {
    problem_type: "https://example.com/errors/validation-error".into(),
    title: "Validation Error".into(),
    status: 400,
    detail: e.to_string(),
    instance: None,
}))?;
```

---

## 5. Module Layout

```
inbound/src/http/
├── mod.rs        # pub mod dto, errors, orders, router;
├── router.rs
├── orders.rs
├── dto.rs
└── errors.rs     # ProblemDetail + ApiError + From<DomainError>
```

`inbound/Cargo.toml` additions (layer-specific, not in workspace):

```toml
actix-web  = "4"
validator  = { version = "0.18", features = ["derive"] }
serde_json = "1"
```
