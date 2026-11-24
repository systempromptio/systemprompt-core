//! The identity HTTP surface contract.
//!
//! The `identity-http` member contributes an [`IdentitySurface`]: one builder
//! the API mount calls with the route-scoped state of `identity-http`
//! ([`IdentityRouteState`]). The member owns the handlers; the API owns the
//! layering around what the builder returns — each router's authorization
//! policy, request-context kind, rate scope and budget, protection layers and
//! the route group it is served in.
//!
//! [`IdentityRouteState::resolved`] is the kernel resolution scoped to
//! `identity-http`: it serves the open state and the sealed values conferred
//! on `identity-http` (the token authority, the signing keys and the
//! users member's sign-in session view) and nothing else sealed.
//! [`IdentityHostHandles`] carries the live process handles the kernel state
//! does not hold, and the [`IdentityConfigView`]: the runtime-configuration
//! facts the identity surface reads, never the whole configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use ipnet::IpNet;
use systemprompt_extension::prelude::{MissingDependency, Resolved};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::{AuthorizeConfig, TrustedIssuer, UiSessionConfig};
use systemprompt_models::auth::JwtAudience;
use systemprompt_traits::{AnalyticsProvider, UserProvider};

use crate::contract::SsoProviders;

/// Builds the OAuth, SSO, self-service session and API-key routes from the
/// `identity-http` route state.
pub trait IdentitySurface: Send + Sync + std::fmt::Debug {
    fn build(&self, state: IdentityRouteState<'_>) -> Result<IdentityRoutes, IdentitySurfaceError>;
}

pub type DynIdentitySurface = Arc<dyn IdentitySurface>;

/// The state the API mount hands an [`IdentitySurface`].
#[derive(Debug)]
pub struct IdentityRouteState<'a> {
    pub resolved: Resolved<'a>,
    pub host: IdentityHostHandles,
}

/// The runtime-configuration facts the identity surface reads.
///
/// They are the external URL and site name (`WebAuthn` relying party, SSO redirects, the session
/// cookie and the bridge token endpoint), the server URL (RFC 7592 client
/// configuration URIs), the JWT issuer, audiences and token lifetimes, the
/// resource audiences, trusted issuers and ID-JAG lifetime token exchange
/// checks, the trusted proxies, the registration and discovery switches, the
/// login page and authorize policy, and the API-key lifetime cap.
#[derive(Debug, Clone)]
pub struct IdentityConfigView {
    pub sitename: String,
    pub api_external_url: String,
    pub api_server_url: String,
    pub jwt_issuer: String,
    pub jwt_audiences: Vec<JwtAudience>,
    pub jwt_access_token_expiration: i64,
    pub jwt_refresh_token_expiration: i64,
    pub allowed_resource_audiences: Vec<String>,
    pub trusted_issuers: Vec<TrustedIssuer>,
    pub id_jag_ttl_secs: i64,
    pub trusted_proxies: Vec<IpNet>,
    pub allow_registration: bool,
    pub allow_dynamic_client_registration: bool,
    pub login_page_url: Option<String>,
    pub authorize: AuthorizeConfig,
    pub ui_session: UiSessionConfig,
    pub api_key_max_lifetime_days: Option<u32>,
}

impl IdentityConfigView {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            sitename: config.sitename.clone(),
            api_external_url: config.api_external_url.clone(),
            api_server_url: config.api_server_url.clone(),
            jwt_issuer: config.jwt_issuer.clone(),
            jwt_audiences: config.jwt_audiences.clone(),
            jwt_access_token_expiration: config.jwt_access_token_expiration,
            jwt_refresh_token_expiration: config.jwt_refresh_token_expiration,
            allowed_resource_audiences: config.allowed_resource_audiences.clone(),
            trusted_issuers: config.trusted_issuers.clone(),
            id_jag_ttl_secs: config.id_jag_ttl_secs,
            trusted_proxies: config.trusted_proxies.clone(),
            allow_registration: config.allow_registration,
            allow_dynamic_client_registration: config.allow_dynamic_client_registration,
            login_page_url: config.login_page_url.clone(),
            authorize: config.authorize,
            ui_session: config.ui_session,
            api_key_max_lifetime_days: config.api_key_max_lifetime_days,
        }
    }
}

/// The live process handles the identity surface reads beyond the kernel state.
///
/// The identity configuration view, the analytics extractor, the user provider
/// and the SSO providers the `identity` member built.
#[derive(Clone)]
pub struct IdentityHostHandles {
    pub config: Arc<IdentityConfigView>,
    pub analytics: Arc<dyn AnalyticsProvider>,
    pub users: Option<Arc<dyn UserProvider>>,
    pub sso: Arc<SsoProviders>,
}

impl std::fmt::Debug for IdentityHostHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityHostHandles")
            .field("sso", &self.sso)
            .finish_non_exhaustive()
    }
}

/// What an [`IdentitySurface`] builds, each router still to be protected and
/// mounted by the API.
#[derive(Debug)]
pub struct IdentityRoutes {
    pub oauth_public: Router,
    pub oauth_authenticated: Router,
    pub oauth_revocation: Router,
    pub sso: Router,
    pub passkey_link_page: Router,
    pub wellknown: Router,
    pub sessions: Router,
    pub api_keys: Router,
    pub bridge_auth: Router,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum IdentitySurfaceError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
    #[error("the identity routes need a user provider")]
    MissingUserProvider,
}
