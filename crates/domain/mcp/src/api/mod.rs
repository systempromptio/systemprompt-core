use axum::Router;

pub mod routes;

pub use routes::*;

pub fn registry_router(app_context: &systemprompt_runtime::AppContext) -> Router {
    registry::router(app_context)
}
