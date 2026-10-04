# Domain Layer Reference

The `domain` crate must list **no infrastructure crates** in its `Cargo.toml`. Pure Rust only.

---

## Step 1: Domain Model

```rust
// domain/src/model/order.rs
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: OrderId,
    pub customer_id: CustomerId,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(pub uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CustomerId(pub uuid::Uuid);

#[derive(Debug, Clone, PartialEq)]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Shipped,
}
```

Rules:
- Entities are plain Rust structs/enums — no derives from infrastructure crates
- Value objects (e.g. `OrderId`) wrap primitives in newtypes for type safety
- Add test fixtures inside `#[cfg(test)]` blocks:

```rust
#[cfg(test)]
impl Order {
    pub fn fixture() -> Self {
        Order {
            id: OrderId(uuid::Uuid::new_v4()),
            customer_id: CustomerId(uuid::Uuid::new_v4()),
            status: OrderStatus::Pending,
        }
    }
}
```

---

## Step 2: Domain Errors

```rust
// domain/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Order not found: {0}")]
    OrderNotFound(uuid::Uuid),
    #[error("Invalid status transition")]
    InvalidStatusTransition,
    #[error("Infrastructure error: {0}")]
    Infrastructure(String),
}
```

---

## Step 3: Outbound Port (Repository Trait)

The domain *defines* what it needs from infrastructure. Adapters *implement* this.

No `#[async_trait]` required with Rust ≥ 1.75 and static dispatch.

```rust
// domain/src/ports/outbound.rs
use crate::{error::DomainError, model::order::Order};
use uuid::Uuid;

// Requires mockall ≥ 0.12 for impl Trait + async fn support
#[cfg_attr(test, mockall::automock)]
pub trait OrderRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Order, DomainError>;
    async fn save(&self, order: &Order) -> Result<(), DomainError>;
    async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError>;
}
```

> **If you need `dyn OrderRepository`** (runtime polymorphism), add `#[async_trait::async_trait]`
> and replace `impl Iterator<Item = Order>` with `Box<dyn Iterator<Item = Order>>`.
> The trait then becomes object-safe at the cost of heap allocation.

---

### `ports/mod.rs` — combining multiple traits into a supertrait

When the application has two or more port traits (e.g. `AuthRepository` + `CustomerRepository`),
combine them in `domain/src/ports/mod.rs` so that all layers share a single bound:

```rust
// domain/src/ports/mod.rs
mod auth;
mod customer;

pub use auth::AuthRepository;
pub use customer::CustomerRepository;

/// Supertrait — the single outbound adapter implements this.
/// `ping` gives health-check ability without a separate port.
pub trait AppRepository: AuthRepository + CustomerRepository + Send + Sync + 'static {
    async fn ping(&self) -> bool;
}

// Conditionally re-export mocks so domain tests can import them directly.
#[cfg(test)]
pub use auth::MockAuthRepository;  // fine: AuthRepository has no RPIT methods
```

The outbound adapter implements each trait separately, then satisfies the supertrait:

```rust
impl AuthRepository for AppDatabase { /* … */ }
impl CustomerRepository for AppDatabase { /* … */ }
impl AppRepository for AppDatabase {
    async fn ping(&self) -> bool { self.connection.ping().await.is_ok() }
}
```

> **Why in `domain` not `inbound`?** Placing the supertrait in `domain` means `outbound` satisfies
> it without importing anything from `inbound`, preserving the one-way dependency rule.

---

### mockall limitation — RPIT return types

`#[cfg_attr(test, mockall::automock)]` **cannot** be placed on a trait containing a method that
returns `impl Trait` in return position, e.g.:

```rust
// ❌ Will NOT compile with #[automock]
async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError>;
```

**Rule:** Only apply `automock` to traits whose every method has a concrete return type.
For RPIT traits, write a hand-written fake in a `#[cfg(test)]` module:

```rust
// domain/src/ports/outbound.rs
// Note: no #[cfg_attr(test, mockall::automock)] here
pub trait OrderRepository: Send + Sync + 'static {
    async fn find_by_id(&self, id: Uuid) -> Result<Order, DomainError>;
    async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError>; // RPIT
    async fn save(&self, order: &Order) -> Result<(), DomainError>;
}

#[cfg(test)]
pub struct FakeOrderRepository {
    pub orders: Vec<Order>,
}

#[cfg(test)]
impl OrderRepository for FakeOrderRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Order, DomainError> {
        self.orders.iter().find(|o| o.id.0 == id)
            .cloned()
            .ok_or(DomainError::OrderNotFound(id))
    }
    async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError> {
        Ok(self.orders.clone().into_iter()) // concrete type — compiles correctly
    }
    async fn save(&self, _o: &Order) -> Result<(), DomainError> { Ok(()) }
}
```

For sync traits **without** RPIT, `automock` works as usual:

```rust
#[cfg_attr(test, mockall::automock)]
pub trait TokenService: Send + Sync + 'static {
    fn generate_token(&self, user_id: Uuid) -> Result<String, DomainError>; // concrete ✓
    fn verify_token(&self, token: &str)      -> Result<TokenClaims, DomainError>;
}
```

---

## Step 4: Use Case Functions

Each use case is a plain exported async function in `domain/src/use_cases/`. The repository
is received as a generic parameter — static dispatch, no struct, no trait impl needed.

```rust
// domain/src/use_cases/orders.rs
use crate::{
    error::DomainError,
    model::order::{Order, OrderStatus},
    ports::outbound::OrderRepository,
};
use uuid::Uuid;

pub async fn get_order<R: OrderRepository>(repo: &R, id: Uuid) -> Result<Order, DomainError> {
    repo.find_by_id(id).await
}

pub async fn confirm_order<R: OrderRepository>(repo: &R, id: Uuid) -> Result<Order, DomainError> {
    let mut order = repo.find_by_id(id).await?;
    if order.status != OrderStatus::Pending {
        return Err(DomainError::InvalidStatusTransition);
    }
    order.status = OrderStatus::Confirmed;
    repo.save(&order).await?;
    Ok(order)
}

pub async fn list_orders<R: OrderRepository>(
    repo: &R,
) -> Result<impl Iterator<Item = Order>, DomainError> {
    repo.find_all().await
}
```

---

## `domain/src/lib.rs`

```rust
#![allow(async_fn_in_trait)] // Rust ≥ 1.75 — no `async-trait` crate needed

pub mod error;
pub mod model;
pub mod ports;
pub mod use_cases;
```

## `domain/Cargo.toml`

```toml
[package]
name = "domain"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror.workspace = true
uuid.workspace = true
tokio.workspace = true

[dev-dependencies]
mockall = "0.12"
tokio = { workspace = true, features = ["macros", "rt"] }
```
