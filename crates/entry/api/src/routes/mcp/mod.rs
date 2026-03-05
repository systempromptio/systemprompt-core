pub mod registry;

use axum::Router;
use axum::routing::get;

pub fn registry_router() -> Router {
    Router::new().route("/", get(registry::handle_mcp_registry))
}
