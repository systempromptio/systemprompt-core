use axum::Router;

use crate::OAuthState;

pub mod routes;
pub mod wellknown;

pub fn router(state: OAuthState) -> Router {
    Router::new()
        .merge(routes::router())
        .with_state(state)
}

pub fn public_router(state: OAuthState) -> Router {
    Router::new()
        .merge(routes::public_router())
        .with_state(state)
}

pub fn authenticated_router(state: OAuthState) -> Router {
    Router::new()
        .merge(routes::authenticated_router())
        .with_state(state)
}
