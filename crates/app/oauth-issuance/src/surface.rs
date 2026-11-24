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
//! does not hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::prelude::{MissingDependency, Resolved};
use systemprompt_manifest::Config;
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

/// The live process handles the identity surface reads beyond the kernel state.
///
/// The runtime configuration the API also hands every request as an
/// extension, the analytics extractor, the user provider and the SSO providers
/// the `identity` member built.
#[derive(Clone)]
pub struct IdentityHostHandles {
    pub config: Arc<Config>,
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
