//! The instance's router. It serves the health route only.

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use systemprompt_models::ApiPaths;

use crate::state::InstanceState;

pub(crate) fn router(state: Arc<InstanceState>) -> Router {
    Router::new()
        .route(ApiPaths::HEALTH, get(health))
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}
