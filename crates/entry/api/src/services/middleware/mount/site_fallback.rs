//! The composition's site fallback as the web root's fallback: public policy
//! and public context only, behind the site-auth layer when a member declares
//! site auth, layered whole, and answering `404` for any path that falls
//! under a reserved entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ProtectionLayer, RateBudget, RateLimit, RouteGroup, RouteProtection,
};

use super::{MountError, MountLayers, RootFallback, protect_root_fallback, reserved_guard};
use crate::services::middleware::rate_limit::LayerReach;

pub const SITE_FALLBACK_RATE: RateLimit = RateLimit::new("site_fallback", RateBudget::Content);

pub fn protect_site_fallback(
    fallback: Router,
    layers: &MountLayers,
) -> Result<RootFallback, MountError> {
    let protection = site_fallback_protection(layers.guards.site_auth.is_some());
    let owned = protect_root_fallback(fallback, &protection, layers, RouteGroup::Extensions)?;
    Ok(RootFallback {
        router: reserved_guard::guarded(owned.router, LayerReach::Router),
        group: owned.group,
    })
}

impl RootFallback {
    #[must_use]
    pub fn beside(self, routes: Router) -> Self {
        Self {
            router: routes.merge(self.router),
            group: self.group,
        }
    }
}

#[must_use]
pub fn site_fallback_protection(site_auth: bool) -> RouteProtection {
    let protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
        .with_rate_limit(SITE_FALLBACK_RATE);
    if site_auth {
        protection.with_layer(ProtectionLayer::SiteAuth)
    } else {
        protection
    }
}
