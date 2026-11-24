//! The identity HTTP surface: the OAuth 2.0 / OIDC endpoints ([`oauth`]),
//! browser-redirect SSO ([`sso`]), the caller's own session revocation
//! ([`users`]) and admin API-key management (`admin_keys`).
//!
//! [`IdentityHttpSurface`] builds them from the `identity-http` route state.
//! It reads, scoped to `identity-http`: the token authority and the signing
//! keys (both host seeds conferred on `identity-http`, the issuer), the users
//! member's sign-in session view (conferred), the OAuth repositories (a host
//! seed conferred on `identity-http`), the open subject-provider set and MCP
//! registry, and the member's own sealed [`IdentityHttpRepositories`]. Without
//! an MCP registry an authorization request naming a `resource` is refused with
//! `invalid_target`; it never falls back to the client's registered scopes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin_keys;
pub mod oauth;
pub mod sso;
pub mod users;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use systemprompt_database::DbPool;
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_oauth::services::WebAuthnService;
use systemprompt_oauth::{OAuthSigning, OAuthState};
use systemprompt_oauth_issuance::JwksCache;
use systemprompt_oauth_issuance::surface::{
    IdentityRouteState, IdentityRoutes, IdentitySurface, IdentitySurfaceError,
};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{DynSignInSessions, McpRegistryProvider};
use systemprompt_users::{ApiKeyLifetime, ApiKeyService, UserRepository};

use admin_keys::ApiKeysState;
use oauth::discovery::DiscoveryState;

/// The repositories the identity routes read that no other member provides,
/// built once in the `identity-http` member's provide.
#[derive(Debug, Clone)]
pub struct IdentityHttpRepositories {
    pub users: Arc<UserRepository>,
}

/// The `identity-http` member's [`IdentitySurface`].
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityHttpSurface;

impl IdentitySurface for IdentityHttpSurface {
    fn build(&self, state: IdentityRouteState<'_>) -> Result<IdentityRoutes, IdentitySurfaceError> {
        let r = &state.resolved;
        let host = state.host;
        let users = host
            .users
            .ok_or(IdentitySurfaceError::MissingUserProvider)?;
        let sessions = r.get::<DynSignInSessions>()?;
        let oauth_repositories = r.get::<Arc<OAuthRepositories>>()?;
        let own = r.get::<Arc<IdentityHttpRepositories>>()?;
        let mcp_registry = r.optional::<Arc<dyn McpRegistryProvider>>();
        let db = r.get::<DbPool>()?;

        let mut oauth_state = OAuthState::new(
            oauth_repositories.oauth.clone(),
            Arc::clone(&host.analytics),
            Arc::clone(&sessions),
            Arc::clone(&users),
            OAuthSigning {
                keys: r.get::<SigningKeys>()?,
                authority: r.get::<Arc<dyn TokenAuthority>>()?,
            },
        );
        if let Some(registry) = &mcp_registry {
            oauth_state = oauth_state.with_mcp_registry(Arc::clone(registry));
        } else {
            tracing::info!("No MCP registry; authorization requests naming a resource are refused");
        }
        match WebAuthnService::new(
            &host.config,
            oauth_repositories.oauth.clone(),
            Arc::clone(&users),
        ) {
            Ok(service) => oauth_state = oauth_state.with_webauthn(Arc::new(service)),
            Err(e) => {
                tracing::warn!(error = %e, "WebAuthn is not configured; passkey routes will refuse");
            },
        }

        let sso = sso::SsoRouteState::build(&host.config, &db, &oauth_state, &host.sso);
        let discovery = DiscoveryState::from_config(&host.config, mcp_registry);
        let api_keys = ApiKeysState {
            keys: Arc::new(ApiKeyService::new(
                Arc::clone(&own.users),
                ApiKeyLifetime::from_max_days(host.config.api_key_max_lifetime_days),
            )),
            subjects: r.get::<Arc<SubjectProviderSet>>()?,
        };

        Ok(IdentityRoutes {
            oauth_public: oauth::public_router(Arc::new(JwksCache::new()))
                .with_state(oauth_state.clone()),
            oauth_authenticated: oauth::authenticated_router().with_state(oauth_state.clone()),
            oauth_revocation: oauth::revocation_router().with_state(oauth_state),
            sso: sso::router(sso),
            passkey_link_page: Router::new().route(
                "/auth/link-passkey",
                get(oauth::webauthn::link::link_passkey_page),
            ),
            wellknown: oauth::wellknown_routes(discovery),
            sessions: users::router(sessions),
            api_keys: admin_keys::router(api_keys),
        })
    }
}
