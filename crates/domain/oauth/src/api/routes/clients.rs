use super::client;
use crate::OAuthState;
use axum::routing::{delete, get, post, put};
use axum::Router;

pub fn router() -> Router<OAuthState> {
    Router::new()
        .route("/", get(client::list::list_clients))
        .route("/", post(client::create::create_client))
        .route("/{client_id}", get(client::get::get_client))
        .route("/{client_id}", put(client::update::update_client))
        .route("/{client_id}", delete(client::delete::delete_client))
}
