use super::{clients, health, endpoints, webauthn};
use systemprompt_oauth::OAuthState;
use axum::routing::{get, post};
use axum::Router;

pub fn router() -> Router<OAuthState> {
    Router::new()
        .merge(public_router())
        .merge(authenticated_router())
}

pub fn public_router() -> Router<OAuthState> {
    Router::new()
        .route("/health", get(health::handle_health_api))
        .route("/session", post(endpoints::anonymous::generate_anonymous_token))
        .route(
            "/webauthn/complete",
            get(endpoints::webauthn_complete::handle_webauthn_complete),
        )
        .route("/token", post(endpoints::token::handle_token))
        .route("/authorize", get(endpoints::authorize::handle_authorize_get))
        .route("/authorize", post(endpoints::authorize::handle_authorize_post))
        .route("/callback", get(endpoints::callback::handle_callback))
        .route("/register", post(endpoints::register::register_client))
        .route(
            "/register/{client_id}",
            get(endpoints::client_config::get_client_configuration),
        )
        .route(
            "/register/{client_id}",
            axum::routing::put(endpoints::client_config::update_client_configuration),
        )
        .route(
            "/register/{client_id}",
            axum::routing::delete(endpoints::client_config::delete_client_configuration),
        )
        .route(
            "/webauthn/register/start",
            post(webauthn::register::start_register),
        )
        .route(
            "/webauthn/register/finish",
            post(webauthn::register::finish_register),
        )
        .route(
            "/webauthn/auth/start",
            post(webauthn::authenticate::start_auth),
        )
        .route(
            "/webauthn/auth/finish",
            post(webauthn::authenticate::finish_auth),
        )
}

pub fn authenticated_router() -> Router<OAuthState> {
    Router::new()
        .nest("/clients", clients::router())
        .route("/introspect", post(endpoints::introspect::handle_introspect))
        .route("/revoke", post(endpoints::revoke::handle_revoke))
        .route("/userinfo", get(endpoints::userinfo::handle_userinfo))
        .route("/consent", get(endpoints::consent::handle_consent_get))
        .route("/consent", post(endpoints::consent::handle_consent_post))
}
