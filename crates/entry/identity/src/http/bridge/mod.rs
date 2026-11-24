//! The bridge credential exchange `identity-http` serves under
//! `/v1/gateway/auth/bridge/*`: PAT, session-code and device-link exchanges,
//! the bridge's OAuth-client provisioning and the capability list.
//!
//! [`BridgeAuthState`] is built from the `identity-http` route state: the
//! request authenticator, the signing keys and OAuth repositories conferred
//! on `identity-http`, the users member's sign-in sessions, the member's own
//! API-key service and the host's configuration, analytics extractor and user
//! provider. Bridge tokens are minted through the one signing-keys seed the
//! identity routes already hold. The API mount owns the `bridge_auth` rate
//! scope, the confinement and the access log around these routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod device_link;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use systemprompt_extension::router::ApiHttpError;
use systemprompt_manifest::Config;
use systemprompt_models::ApiPaths;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::{OauthError, OauthErrorKind};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{AnalyticsProvider, DynSignInSessions, UserProvider};
use systemprompt_users::{ApiKeyService, UserError};

/// What every bridge credential-exchange handler reads.
#[derive(Clone)]
pub struct BridgeAuthState {
    pub authenticator: DynRequestAuthenticator,
    pub keys: SigningKeys,
    pub oauth: OAuthRepository,
    pub sessions: DynSignInSessions,
    pub users: Arc<dyn UserProvider>,
    pub api_keys: Arc<ApiKeyService>,
    pub analytics: Arc<dyn AnalyticsProvider>,
    pub config: Arc<Config>,
}

impl std::fmt::Debug for BridgeAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeAuthState").finish_non_exhaustive()
    }
}

pub fn router(state: &BridgeAuthState) -> Router {
    let pat = state.clone();
    let session = state.clone();
    let session_pat = state.clone();
    let oauth_client = state.clone();
    Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_PAT),
            post(move |request| {
                let state = pat.clone();
                async move { auth::pat(state, request).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_SESSION),
            post(move |caller_ip, headers, body| {
                let state = session.clone();
                async move { auth::session(state, caller_ip, headers, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_SESSION_PAT),
            post(move |body| {
                let state = session_pat.clone();
                async move { device_link::session_pat(state, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT),
            post(move |request| {
                let state = oauth_client.clone();
                async move { auth::provision_oauth_client(state, request).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_CAPABILITIES),
            get(auth::capabilities),
        )
}

pub(crate) fn users_error(err: UserError) -> ApiHttpError {
    ApiHttpError::from(ApiError::from(err))
}

pub(crate) fn oauth_error(err: OauthError) -> ApiHttpError {
    if matches!(
        err,
        OauthError::CodeNotFound(_)
            | OauthError::TokenNotFound(_)
            | OauthError::ClientNotFound(_)
            | OauthError::UserNotFound(_)
    ) {
        return ApiHttpError::not_found(err.to_string());
    }
    ApiHttpError::from(match err.kind() {
        OauthErrorKind::InvalidRequest
        | OauthErrorKind::InvalidClientMetadata
        | OauthErrorKind::ExpiredChallenge
        | OauthErrorKind::InvalidCredential => ApiError::bad_request(err.to_string()),
        OauthErrorKind::UsernameUnavailable | OauthErrorKind::EmailExists => {
            ApiError::conflict(err.to_string())
        },
        OauthErrorKind::InvalidClient => ApiError::unauthorized("Client authentication failed"),
        OauthErrorKind::AuthenticationFailed => ApiError::unauthorized("Authentication failed"),
        OauthErrorKind::InvalidGrant
        | OauthErrorKind::InvalidToken
        | OauthErrorKind::AccessDenied => ApiError::unauthorized(err.to_string()),
        OauthErrorKind::NotFound => ApiError::not_found(err.to_string()),
        OauthErrorKind::ServerError => ApiError::internal("Authorization operation failed", err),
    })
}

pub(crate) fn credential_error(err: ContextExtractionError) -> ApiHttpError {
    ApiHttpError::from(match err {
        ContextExtractionError::InvalidToken(source) => {
            ApiError::unauthorized("Invalid or expired token").with_source(source)
        },
        e @ (ContextExtractionError::MissingHeader(_)
        | ContextExtractionError::MissingAuthHeader
        | ContextExtractionError::Revoked
        | ContextExtractionError::MissingSessionId
        | ContextExtractionError::MissingUserId) => ApiError::unauthorized(e.to_string()),
        e @ (ContextExtractionError::MissingContextId
        | ContextExtractionError::InvalidHeaderValue { .. }
        | ContextExtractionError::InvalidUserId(_)) => ApiError::bad_request(e.to_string()),
        e @ ContextExtractionError::ForbiddenHeader { .. } => ApiError::forbidden(e.to_string()),
        e @ ContextExtractionError::UserNotFound(_) => ApiError::not_found(e.to_string()),
        e @ ContextExtractionError::PayloadTooLarge { .. } => {
            ApiError::payload_too_large(e.to_string())
        },
        e @ ContextExtractionError::DatabaseError { .. } => {
            ApiError::internal("Request context lookup failed", e)
        },
    })
}
