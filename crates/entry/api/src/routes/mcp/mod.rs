//! MCP registry routes.
//!
//! [`registry_router`] mounts the read-only MCP server registry endpoint backed
//! by [`registry::handle_mcp_registry`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod registry;

use axum::Router;
use axum::routing::get;
use systemprompt_runtime::AppContext;

pub fn registry_router(ctx: &AppContext) -> Router {
    Router::new()
        .route("/", get(registry::handle_mcp_registry))
        .with_state(ctx.clone())
}
