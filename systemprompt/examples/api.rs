//! Stub showing how to construct an `ApiServer` from a bare `axum::Router`.
//!
//! Run with: `cargo run -p systemprompt --example api --features api`
//!
//! In a real deployment you would build the router via the runtime's
//! `AppContext` plumbing: extensions return an `ExtensionRouter` from
//! `Extension::router(&ctx)` and the runtime mounts them onto the server (see
//! the `systemprompt-template` web extension). `ApiServer::new` shown here is
//! the low-level surface those higher layers build on.

use systemprompt::api::ApiServer;
use systemprompt::prelude::Router;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();
    let router: Router = Router::new();
    let server = ApiServer::new(router, None);
    tracing::info!(?server, "constructed ApiServer");
}
