# Outbound Adapter Reference (SeaORM v2)

Outbound adapters implement repository port traits defined in `domain`. They import `domain`
and infrastructure crates — never `inbound`.

Keep adapter boundaries explicit:
- SeaORM entities and active models stay inside outbound.
- Domain models cross the boundary only via mapping code (`From` impls or mapper fns).
- `DbErr` and transport-specific failures are translated to domain errors in the adapter.

---

## Single-struct, all-ports pattern

For applications with a single database, implement all repository port traits on one struct
that wraps the connection pool. Each port gets its own `impl` block.

```rust
// outbound/src/database/connection.rs
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AppDatabase {
    pub(crate) connection: DatabaseConnection,  // pub(crate) — never leaks outside outbound
}

impl AppDatabase {
    pub async fn new() -> Self {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
        let mut opts = ConnectOptions::new(url);
        opts.max_connections(50)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(8))
            .acquire_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(300));
        let conn = Database::connect(opts).await
            .unwrap_or_else(|e| { eprintln!("DB connect failed: {e}"); std::process::exit(1) });
        Self { connection: conn }
    }

    pub async fn run_migrations(&self) {
        use migration::{Migrator, MigratorTrait};
        Migrator::up(&self.connection, None).await
            .unwrap_or_else(|e| { eprintln!("Migration failed: {e}"); std::process::exit(1) });
    }

    pub async fn ping(&self) -> Result<(), sea_orm::DbErr> {
        self.connection.ping().await
    }
}
```

Repository traits are implemented as separate `impl AppDatabase` blocks:

```rust
// outbound/src/repositories/order.rs
use domain::{error::DomainError, model::Order, ports::OrderRepository};
use tracing::error;
use crate::database::{connection::AppDatabase, models::order as order_entity};

impl OrderRepository for AppDatabase {
    tracing::instrument(skip(self))
    async fn find_by_id(&self, id: i64) -> Result<Option<Order>, DomainError> {
        order_entity::Entity::find_by_id(id)
            .one(&self.connection).await
            .map_err(|err| {
                error!(error = %err, "DB error finding order by id {id}");

                DomainError::Infrastructure("db".into())
            })
            .map(|m| m.map(Order::from))
    }

    tracing::instrument(skip(self))
    async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError> {
        let rows = order_entity::Entity::find()
            .all(&self.connection).await
            .map_err(|err| {
                error!(error = %err, "DB error finding all orders");

                DomainError::Infrastructure("db".into())
            })?;
        Ok(rows.into_iter().map(Order::from))
    }
}
```

`outbound/src/lib.rs` exports only `AppDatabase`:

```rust
mod database;
pub use database::AppDatabase;
```

> **Advantage:** no per-repository `new(db)` boilerplate; no cloning `DatabaseConnection`
> per repository. `AppDatabase` is `Clone` (inner pool is `Arc`-backed), so it can be
> passed as `web::Data<AppState<…>>` without extra wrappers.

> **Alternative (separate struct per repo):** Preferred when repositories are tested
> independently or when a bounded context is split into its own crate later.
> See the classic `SeaOrmOrderRepository` example below.

---

## SeaORM CLI: Generate Entities

```bash
cargo install sea-orm-cli
sea-orm-cli generate entity \
  -u postgres://user:pass@localhost/db \
  -o outbound/src/db/entities
```

- Generated files in `outbound/src/db/entities/` — **never edit by hand**, regenerate instead
- Entity types never cross crate boundaries

---

## SeaORM Migrations

See [migrations.md](./migrations.md) for full setup. Summary:
- Separate `migration/` crate at workspace root (`sea-orm-cli migrate init`)
- Only dependency: `sea-orm-migration` — no domain/app imports
- `Migrator::up(&db, None).await?` runs on startup in `app/src/main.rs`

---

## SeaORM Entity

Define the entity in `outbound/src/db/entities/`. These types must **never leave** the `outbound` crate.

```rust
// outbound/src/db/entities/order.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

---

## Mappers

Translate between SeaORM `Model` and domain types at the repository boundary. Keep mapper code
in a dedicated module (`mappers.rs`) so repositories stay focused on query logic.

Prefer `From` / `Into` impls when mappings are straightforward (`model.into()`). Use explicit
mapper functions for transformations that require validation, lossy conversion, or fallback rules.

```rust
// outbound/src/database/mappers.rs
use domain::model::order::{Order, OrderId, OrderStatus};
use super::models::order;

impl From<order::Model> for Order {
    fn from(m: order::Model) -> Self {
        Order {
            id:          OrderId(m.id),
            customer_id: m.customer_id,
            status:      OrderStatus::from(m.status),
        }
    }
}

impl From<OrderStatus> for String {
    fn from(s: OrderStatus) -> Self {
        match s {
            OrderStatus::Pending   => "pending".into(),
            OrderStatus::Confirmed => "confirmed".into(),
            OrderStatus::Shipped   => "shipped".into(),
        }
    }
}

impl From<String> for OrderStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "confirmed" => OrderStatus::Confirmed,
            "shipped"   => OrderStatus::Shipped,
            _           => OrderStatus::Pending,
        }
    }
}
```

Call at the repository boundary:

```rust
let row: order::Model = order_entity::Entity::find_by_id(id).one(&self.db).await?
    .ok_or(DomainError::OrderNotFound(id))?;
let domain_order: Order = row.into(); // From impl invoked here
```

### Error mapping at the boundary

Repository implementations should map infrastructure errors immediately:

```rust
fn is_unique_violation(err: &sea_orm::DbErr) -> bool {
    let msg = err.to_string();
    msg.contains("duplicate key")
        || msg.contains("Duplicate entry")
        || msg.contains("UNIQUE constraint failed")
}

// in repository method
.map_err(|e| {
    if is_unique_violation(&e) {
        return DomainError::Conflict("resource already exists".into());
    }
    DomainError::Infrastructure("database error".into())
})
```

Prefer typed database error variants when available; use message fallback only for cross-database
portability.

---

## Repository Implementation

```rust
// outbound/src/db/order_repository.rs
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait};
use domain::{
    error::DomainError,
    model::order::Order,
    ports::outbound::OrderRepository,
};
use uuid::Uuid;
use tracing::error;
use super::{entities::order::Entity as OrderEntity, mappers};

pub struct SeaOrmOrderRepository {
    db: DatabaseConnection,
}

impl SeaOrmOrderRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl OrderRepository for SeaOrmOrderRepository {
    #[tracing::instrument(skip(self))]
    async fn find_by_id(&self, id: Uuid) -> Result<Order, DomainError> {
        OrderEntity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!(error = %e, "DB error finding order by id {id}");

                DomainError::Infrastructure(e.to_string())
            })?
            .map(mappers::to_domain)
            .ok_or(DomainError::OrderNotFound(id))
    }

    // Returns impl Iterator — callers decide whether to collect
    #[tracing::instrument(skip(self))]
    async fn find_all(&self) -> Result<impl Iterator<Item = Order>, DomainError> {
        let rows = OrderEntity::find()
            .all(&self.db)
            .await
            .map_err(|e| {
                error!(error = %e, "DB error finding all orders");
                DomainError::Infrastructure(e.to_string())
            })?;
        Ok(rows.into_iter().map(mappers::to_domain))
    }

    #[tracing::instrument(skip(self, order))]  
    async fn save(&self, order: &Order) -> Result<(), DomainError> {
        // Use upsert: INSERT … ON CONFLICT (id) DO UPDATE.
        // A plain .insert() would fail with a duplicate-key error when
        // saving a mutated order that already exists in the DB.
        use sea_orm::sea_query::OnConflict;
        use entities::order::Column;
        entities::order::Entity::insert(mappers::to_active_model(order))
            .on_conflict(
                OnConflict::column(Column::Id)
                    .update_column(Column::Status)
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(|e| {
                error!(error = %e, "DB error saving order {order:?}");
                DomainError::Infrastructure(e.to_string())
            })?;
        Ok(())
    }
}
```

---

## Module Layout

```
outbound/src/
├── lib.rs                      # pub mod db;
└── db/
    ├── mod.rs                  # pub mod order_repository; mod entities; mod mappers;
    ├── order_repository.rs
    ├── entities/
    │   ├── mod.rs
    │   └── order.rs
    └── mappers.rs
```

## `outbound/Cargo.toml`

```toml
[package]
name = "outbound"
version = "0.1.0"
edition = "2021"

[dependencies]
domain.workspace = true
uuid.workspace   = true
tokio.workspace  = true
# layer-specific — not in workspace
sea-orm = { version = "2", features = ["sqlx-postgres", "runtime-tokio-rustls", "macros"] }
```
