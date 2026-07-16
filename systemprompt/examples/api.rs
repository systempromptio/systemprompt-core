//! Stub showing the early-bind API listener from `systemprompt-api`.
//!
//! Run with: `cargo run -p systemprompt --example api --features api`
//!
//! `bind_and_serve` binds the listener immediately and answers health probes
//! with `{"status":"starting"}`. In a real deployment the runtime builds the
//! full router from `AppContext` (extensions return an `ExtensionRouter` from
//! `Extension::router(&ctx)`) and swaps it in via `EarlyServer::activate`.

use systemprompt::api::services::server::bind_and_serve;
use systemprompt::prelude::Router;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();
    match bind_and_serve("127.0.0.1:0", None).await {
        Ok(early) => {
            tracing::info!(addr = %early.local_addr(), "listener bound, probes report starting");
            early.activate(Router::new());
        },
        Err(err) => {
            tracing::error!(error = %err, "bind failed");
        },
    }
}
