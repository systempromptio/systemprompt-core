//! The bridge credential exchange `identity-http` serves under
//! `/v1/gateway/auth/bridge/*`: PAT, session-code and device-link exchanges,
//! the bridge's OAuth-client provisioning and the capability list.
//!
//! [`BridgeAuthState`] is built from the `identity-http` route state: the
//! request authenticator, the signing keys and OAuth repositories conferred
//! on `identity-http`, the users member's sign-in sessions, the member's own
//! API-key service and the host's identity configuration view, analytics
//! extractor and user provider. Bridge tokens are minted through the one
//! signing-keys seed the identity routes already hold. The API mount owns the
//! `bridge_auth` rate scope, the confinement and the access log around these
//! routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod device_link;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use systemprompt_extension::router::ApiHttpError;
use systemprompt_models::ApiPaths;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth_issuance::surface::IdentityConfigView;
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
    pub config: Arc<IdentityConfigView>,
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
    ApiHttpError::from(ApiError::from(err))
}

pub(crate) fn credential_error(err: ContextExtractionError) -> ApiHttpError {
    ApiHttpError::from(ApiError::from(err))
}
