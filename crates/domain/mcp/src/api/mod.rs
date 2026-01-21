use axum::Router;

pub mod routes;

pub use routes::*;

pub fn registry_router() -> Router {
    registry::router()
}
