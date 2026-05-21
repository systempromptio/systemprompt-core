pub mod registry;

use axum::Router;
use axum::routing::get;
use systemprompt_runtime::AppContext;

pub fn registry_router(ctx: &AppContext) -> Router {
    Router::new()
        .route("/", get(registry::handle_mcp_registry))
        .with_state(ctx.clone())
}
