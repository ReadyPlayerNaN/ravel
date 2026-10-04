# SeaORM Migrations Reference

> Docs: https://www.sea-ql.org/SeaORM/docs/migration/setting-up-migration/

Migrations live in a **separate `migration/` crate** at the workspace root — not inside `outbound`.

---

## Setup

```bash
sea-orm-cli migrate init   # creates migration/ crate
```

Add to workspace:

```toml
# Cargo.toml (root)
[workspace]
members = ["app", "domain", "inbound", "outbound", "migration"]
```

### `migration/Cargo.toml`

```toml
[package]
name    = "migration"
version = "0.1.0"
edition = "2021"

[dependencies]
# Only this — no domain, outbound, inbound, or app imports
sea-orm-migration = { version = "2", features = ["sqlx-postgres", "runtime-tokio-rustls"] }
```

---

## Sample Migration

```rust
// migration/src/m20240101_000001_create_order_table.rs
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str { "m20240101_000001_create_order_table" }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(Order::Table)
                .if_not_exists()
                .col(ColumnDef::new(Order::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Order::CustomerId).uuid().not_null())
                .col(ColumnDef::new(Order::Status).string().not_null())
                .to_owned(),
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Order::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum Order { Table, Id, CustomerId, Status }
```

### Register in `migration/src/lib.rs`

```rust
pub use sea_orm_migration::prelude::*;
mod m20240101_000001_create_order_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20240101_000001_create_order_table::Migration)]
    }
}
```

---

## CLI Commands

```bash
sea-orm-cli migrate up      # apply pending
sea-orm-cli migrate down    # revert last
sea-orm-cli migrate fresh   # drop all + re-apply
```

---

## Run on Startup (`app/src/main.rs`)

```rust
use migration::{Migrator, MigratorTrait};

// after obtaining `db: DatabaseConnection`
Migrator::up(&db, None).await?;
```

`app/Cargo.toml`:
```toml
migration = { path = "../migration" }
```
