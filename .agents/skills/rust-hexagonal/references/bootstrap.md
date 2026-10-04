# Bootstrap / Composition Root Reference

`app/src/main.rs` is the **only** file that imports all concrete types. It constructs the full
dependency graph and starts the server. No other crate should know about more than one layer.

With static dispatch, all types are monomorphized at compile time — no `Arc<dyn Trait>` needed
unless you explicitly want runtime polymorphism.

The composition is usually split across three files in `app/src/`:

| File | Responsibility |
|------|---------------|
| `config.rs` | Read env vars into a typed config struct |
| `state.rs` | Build shared app state and/or typed framework app-data values from config |
| `main.rs` | Wire state into actix-web, bind, and serve |
| `core/` | Infrastructure services that implement domain ports but live in `app` (e.g. `JwtTokenService`) |

---

## `app/src/config.rs`

```rust
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")?,
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
        })
    }
}
```

---

## `app/src/state.rs`

Constructs inbound dependencies by wiring outbound adapters and app-local services.
`app` is the only crate that names concrete adapter types.

```rust
use inbound::state::AppState;
use outbound::db::SeaOrmOrderRepository;
use super::config::AppConfig;

// Name the concrete type once, here.
pub type AppStateImpl = AppState<SeaOrmOrderRepository>;

pub async fn build_state(cfg: &AppConfig) -> anyhow::Result<AppStateImpl> {
    let db = sea_orm::Database::connect(&cfg.database_url).await?;

    // Run pending migrations before accepting traffic
    migration::Migrator::up(&db, None).await?;

    // AppState<R> does not own an Arc — web::Data (actix) or Arc::new (axum)
    // provides shared ownership at the composition root.
    Ok(AppState {
        repo: SeaOrmOrderRepository::new(db),
    })
}
```

If your inbound runtime injects dependencies as separate typed values (instead of one aggregate
state struct), keep the same boundary rule: only `app` names concrete types.

---

## `app/src/main.rs` (actix-web — preferred)

```rust
// Global allocator swap — recommended for production long-running services.
// jemallocator reduces fragmentation under high concurrent load.
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db = outbound::AppDatabase::new().await; // process::exit(1) on connect failure
    db.run_migrations().await;                   // process::exit(1) on migration failure

    let token_service = core::auth::JwtTokenService::new();
    inbound::run(db, token_service).await
}
```

> **`process::exit(1)` on fatal infra errors** (DB connect failure, migration failure) is
> intentional. These conditions are unrecoverable at startup; logging the error and exiting
> immediately is clearer than propagating through `anyhow` or panicking with a backtrace.

---

## `app/src/main.rs` (axum — alternative)

```rust
mod config;
mod state;

use config::AppConfig;
use outbound::db::SeaOrmOrderRepository;
use inbound::http::router::order_router;
use inbound::state::AppState;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let cfg   = AppConfig::from_env()?;
    let state = Arc::new(state::build_state(&cfg).await?);

    let app      = order_router::<SeaOrmOrderRepository>(state);
    let listener = tokio::net::TcpListener::bind((cfg.host.as_str(), cfg.port)).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

---

## `app/Cargo.toml`

```toml
[package]
name    = "app"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "server"
path = "src/main.rs"

[dependencies]
domain.workspace    = true
inbound.workspace   = true
outbound.workspace  = true
migration.workspace = true        # or migrations.workspace when crate name differs
tokio.workspace     = true
# layer-specific — not in workspace
actix-web          = "4"              # or axum = "0.8"
jemallocator       = "0.5"            # recommended: reduce fragmentation under load
dotenvy            = "0.15"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
# Only if JwtTokenService (or similar infra service) lives in app/src/core/:
jsonwebtoken = { version = "10", features = ["aws_lc_rs"] }
```
