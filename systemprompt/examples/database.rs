//! Demonstrates connecting a `DbPool` against a Postgres URL.
//!
//! Run with: `cargo run -p systemprompt --example database --features database`
//! (requires a reachable Postgres at `postgres://localhost/systemprompt`).
//!
//! This shows the standalone connect path. Inside an extension you do not open
//! your own pool — you take the shared one from the context:
//! `ctx.database().as_any().downcast_ref::<Database>()` then `db.pool()`.

use systemprompt::database::Database;
use systemprompt::prelude::DbPool;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/systemprompt".to_owned());

    match Database::new_postgres(&url).await {
        Ok(db) => {
            let pool: DbPool = std::sync::Arc::new(db);
            tracing::info!(has_write_pool = pool.has_write_pool(), "connected");
        },
        Err(err) => {
            tracing::error!(error = %err, "connect failed");
        },
    }
}
