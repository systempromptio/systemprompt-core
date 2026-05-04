//! Stub showing how to construct an `ApiServer` from a bare `axum::Router`.
//!
//! Run with: `cargo run -p systemprompt --example api --features api`
//!
//! In a real deployment you would build the router via the runtime's
//! `AppContext` plumbing; this example focuses on the surface re-exported
//! through the facade.

use systemprompt::api::ApiServer;
use systemprompt::prelude::Router;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();
    let router: Router = Router::new();
    let server = ApiServer::new(router, None);
    tracing::info!(?server, "constructed ApiServer");
}
