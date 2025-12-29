use super::{clients, health, oauth, webauthn};
use axum::routing::{get, post};
use axum::Router;

pub fn router() -> Router<systemprompt_runtime::AppContext> {
    Router::new()
        .merge(public_router())
        .merge(authenticated_router())
}

pub fn public_router() -> Router<systemprompt_runtime::AppContext> {
    Router::new()
        .route("/health", get(health::handle_health_api))
        .route("/session", post(oauth::anonymous::generate_anonymous_token))
        .route(
            "/webauthn/complete",
            get(oauth::webauthn_complete::handle_webauthn_complete),
        )
        .route("/token", post(oauth::token::handle_token))
        .route("/authorize", get(oauth::authorize::handle_authorize_get))
        .route("/authorize", post(oauth::authorize::handle_authorize_post))
        .route("/callback", get(oauth::callback::handle_callback))
        .route("/register", post(oauth::register::register_client))
        .route(
            "/register/{client_id}",
            get(oauth::client_config::get_client_configuration),
        )
        .route(
            "/register/{client_id}",
            axum::routing::put(oauth::client_config::update_client_configuration),
        )
        .route(
            "/register/{client_id}",
            axum::routing::delete(oauth::client_config::delete_client_configuration),
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
        .route("/webauthn/dev-auth", post(webauthn::authenticate::dev_auth))
}

pub fn authenticated_router() -> Router<systemprompt_runtime::AppContext> {
    Router::new()
        .nest("/clients", clients::router())
        .route("/introspect", post(oauth::introspect::handle_introspect))
        .route("/revoke", post(oauth::revoke::handle_revoke))
        .route("/userinfo", get(oauth::userinfo::handle_userinfo))
        .route("/consent", get(oauth::consent::handle_consent_get))
        .route("/consent", post(oauth::consent::handle_consent_post))
}
