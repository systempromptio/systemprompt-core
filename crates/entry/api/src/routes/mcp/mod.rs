pub mod registry;

use axum::routing::get;
use axum::Router;

pub fn registry_router() -> Router {
    Router::new().route("/", get(registry::handle_mcp_registry))
}
