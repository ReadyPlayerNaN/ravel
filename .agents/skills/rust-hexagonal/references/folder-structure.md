# Folder Structure Reference — Rust Hexagonal Architecture

## Full Annotated Tree (Workspace — Layout A)

```
my-app/
├── Cargo.toml                              # workspace root
│
├── domain/                                 # lib — ZERO infrastructure deps
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                          # #![allow(async_fn_in_trait)]; pub mod model; pub mod error; pub mod ports; pub mod use_cases;
│       ├── error.rs                        # DomainError (thiserror)
│       ├── model/
│       │   ├── mod.rs
│       │   ├── order.rs                    # Order, OrderId, OrderStatus
│       │   └── customer.rs
│       ├── ports/
│       │   ├── mod.rs                      # optional supertraits that combine multiple ports
│       │   ├── repository/
│       │   │   └── order.rs                # persistence ports
│       │   └── service/
│       │       └── token.rs                # external-service ports
│       └── use_cases/
│           ├── mod.rs
│           └── order/
│               ├── mod.rs                  # re-export public use case fns
│               ├── create.rs               # pub async fn create_order<R: OrderRepository>(…)
│               └── get.rs
│
├── inbound/                                # lib — HTTP/gRPC/CLI adapters
│   ├── Cargo.toml                          # deps: domain, actix-web (or axum), serde, validator
│   └── src/
│       ├── lib.rs                          # pub async fn run<TS, R>(state: AppState<TS, R>) — HTTP server bootstrap
│       ├── config.rs                       # struct AppState<TS: TokenService, R: AppRepository>
│       └── http/
│           ├── mod.rs
│           ├── router.rs                   # fn configure<TS, R>(cfg) — compose all bounded context scopes under /api/v1
│           ├── error.rs                    # ApiError + ResponseError + From<DomainError> + From<ValidationErrors>
│           ├── middleware/
│           │   └── auth.rs                 # JwtClaims<TS, R> FromRequest extractor
│           ├── health/
│           │   └── mod.rs
│           └── orders/                     # one directory per bounded context
│               ├── mod.rs                  # configure fn
│               ├── handler.rs              # handler fns generic over <TS, R>
│               ├── payload.rs              # #[derive(Deserialize, Validate)] request structs
│               └── response.rs             # #[derive(Serialize)] response structs + From<DomainType>
│
├── outbound/                               # lib — DB/cache adapters
│   ├── Cargo.toml                          # deps: domain, migration, sea-orm
│   └── src/
│       ├── lib.rs                          # pub use database::AppDatabase;
│       └── database/
│           ├── mod.rs
│           ├── connection.rs               # AppDatabase (wraps DatabaseConnection; all ports impl on this struct)
│           ├── models/                     # SeaORM entities — never leave outbound crate
│           │   └── order.rs
│           ├── repositories/               # one file per aggregate root; impl XRepository for AppDatabase
│           │   └── order.rs
│           └── mappers.rs                  # From<entity::Model> for DomainType (and reverse for enums)
│
├── migration/                              # lib — DB migrations (some teams use the name migrations)
│   ├── Cargo.toml                          # deps: sea-orm-migration only — no domain/app imports
│   └── src/
│       ├── lib.rs                          # Migrator struct + MigratorTrait impl
│       └── m20240101_000001_create_order_table.rs
│
└── app/                                    # bin — sole composition root
    ├── Cargo.toml                          # deps: all crates above + jsonwebtoken (if JWT lives here)
    └── src/
        ├── main.rs                         # wire all concrete adapters/services into inbound::run(...)
        └── core/
            └── auth.rs                     # JwtTokenService implements domain::ports::TokenService
```

---

## Workspace `Cargo.toml`

Only list dependencies that are **shared across multiple crates**. Layer-specific deps belong in the crate's own `Cargo.toml`.

```toml
[workspace]
members  = ["domain", "inbound", "outbound", "migration", "app"]
resolver = "2"

[workspace.dependencies]
domain    = { path = "domain" }
inbound   = { path = "inbound" }
outbound  = { path = "outbound" }
migration = { path = "migration" }

# shared across multiple crates
tokio     = { version = "1", features = ["full"] }
uuid      = { version = "1", features = ["v4"] }
thiserror = "2"
serde     = { version = "1", features = ["derive"] }
anyhow    = "1"

# actix-web  ← NOT here: only used in inbound
# sea-orm    ← NOT here: only used in outbound / migration
```

---

## Per-Crate `Cargo.toml` Examples

### `domain/Cargo.toml`

```toml
[package]
name    = "domain"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror.workspace = true
uuid.workspace      = true
# tokio only needed in Layout A (use cases inside domain)
tokio = { workspace = true, optional = true }
# async-trait ONLY if you need dyn Trait object safety

[dev-dependencies]
mockall = "0.12"
tokio   = { workspace = true, features = ["macros", "rt"] }
```

### `inbound/Cargo.toml`

```toml
[package]
name    = "inbound"
version = "0.1.0"
edition = "2021"

[dependencies]
domain.workspace = true
serde.workspace  = true
uuid.workspace   = true
tokio.workspace  = true
# layer-specific — not in workspace
actix-web = "4"       # or: axum = "0.8"
```

### `outbound/Cargo.toml`

```toml
[package]
name    = "outbound"
version = "0.1.0"
edition = "2021"

[dependencies]
domain.workspace = true
uuid.workspace   = true
tokio.workspace  = true
# layer-specific — not in workspace
sea-orm = { version = "2", features = ["sqlx-postgres", "runtime-tokio-rustls", "macros"] }
```

---

## Dependency Matrix

| Crate | May depend on |
|-------|--------------|
| `domain` | nothing (std + thiserror + uuid) |
| `inbound` | `domain` only |
| `outbound` | `domain` only |
| `migration` | nothing (sea-orm-migration only) |
| `app` | all of the above |

**Forbidden edges (never list in `Cargo.toml`):**
- `inbound` → `outbound`
- `outbound` → `inbound`
- `domain` → any adapter or app crate
- `migration` → `domain`, `inbound`, or `outbound`

---

## Layout B variant

Add an `application` crate between `domain` and `app`:

```toml
# workspace Cargo.toml
members = ["domain", "application", "inbound", "outbound", "migration", "app"]
```

- `domain` has no `tokio` dependency
- `application` depends on `domain`; contains `use_cases/`
- `app` depends on `domain`, `application`, `inbound`, `outbound`
