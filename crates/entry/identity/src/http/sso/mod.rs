//! Generic browser-redirect SSO routes, mounted at `ApiPaths::SSO_BASE`.
//!
//! Every registered provider is reachable as `/{id}/start`, `/{id}/callback`
//! (query or form-POST return) and `/{id}/metadata`; the protocol itself is
//! the provider's and the flow is the
//! [`systemprompt_oauth_issuance::sso::SsoDriver`]'s. A deployment
//! that registered its identity provider with other paths aliases them onto
//! the same handlers via [`into_response`]. The providers, auth-event sinks
//! and role resolver are the kernel-built [`SsoProviders`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod callback;
mod metadata;
mod respond;
mod start;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use systemprompt_oauth::OAuthState;

use systemprompt_database::DbPool;
use systemprompt_manifest::Config;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_oauth_issuance::contract::SsoProviders;
use systemprompt_oauth_issuance::sso::SsoDriver;
use systemprompt_security::http::SessionCookie;
use systemprompt_traits::AnalyticsProvider;

pub use respond::{binder_from_headers, into_response, start_response};

#[derive(Clone)]
pub struct SsoRouteState {
    pub driver: SsoDriver,
    pub analytics: Arc<dyn AnalyticsProvider>,
    pub cookie: SessionCookie,
}

impl std::fmt::Debug for SsoRouteState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoRouteState")
            .field("driver", &self.driver)
            .finish_non_exhaustive()
    }
}

impl SsoRouteState {
    pub fn build(config: &Arc<Config>, db: &DbPool, oauth: &OAuthState, sso: &SsoProviders) -> Self {
        let session_service = SessionCreationService::new(
            Arc::clone(oauth.session_provider()),
            Arc::clone(oauth.user_provider()),
            Arc::clone(oauth.token_authority()),
            Arc::clone(config),
        );
        let driver = SsoDriver::new(
            sso,
            db,
            Arc::clone(config),
            oauth.oauth_repository().clone(),
            Arc::new(session_service),
        );
        Self {
            driver,
            analytics: Arc::clone(oauth.analytics_provider()),
            cookie: SessionCookie::from_config(config),
        }
    }
}

pub fn router(state: SsoRouteState) -> Router {
    Router::new()
        .route("/{id}/start", get(start::handle_start))
        .route(
            "/{id}/callback",
            get(callback::handle_callback_query).post(callback::handle_callback_form),
        )
        .route("/{id}/metadata", get(metadata::handle_metadata))
        .with_state(state)
}
