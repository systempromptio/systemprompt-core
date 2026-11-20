//! The declaration a server-owned route group is mounted from.
//!
//! The server's own groups — OAuth, protocol, documents, gateway, static and
//! the rest — are kernel built-ins, not composition contributions. Each is a
//! [`KernelRoutes`]: the router, its base path, its [`RouteProtection`], the
//! [`RouteGroup`] that decides which node roles serve it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, ProtectionLayer, RateBudget,
    RateLimit, RouteGroup, RouteProtection,
};
use systemprompt_provider_contracts::FrameOptions;

/// A server-owned router and the declaration it is mounted from.
#[derive(Debug, Clone)]
pub struct KernelRoutes {
    pub router: Router,
    pub base_path: &'static str,
    pub protection: RouteProtection,
    pub group: RouteGroup,
}

impl KernelRoutes {
    #[must_use]
    pub const fn new(
        router: Router,
        base_path: &'static str,
        policy: AuthzPolicy,
        context: ContextKind,
    ) -> Self {
        Self::protected(router, base_path, RouteProtection::new(policy, context))
    }

    #[must_use]
    pub const fn protected(
        router: Router,
        base_path: &'static str,
        protection: RouteProtection,
    ) -> Self {
        Self {
            router,
            base_path,
            protection,
            group: RouteGroup::Extensions,
        }
    }

    #[must_use]
    pub const fn with_rate_limit(mut self, rate_limit: RateLimit) -> Self {
        self.protection.rate_limit = Some(rate_limit);
        self
    }

    #[must_use]
    pub const fn with_body_limit(mut self, bytes: usize) -> Self {
        self.protection.body_limit = Some(bytes);
        self
    }

    #[must_use]
    pub fn with_layer(mut self, layer: ProtectionLayer) -> Self {
        self.protection = self.protection.with_layer(layer);
        self
    }

    #[must_use]
    pub const fn with_frame_options(mut self, frame_options: FrameOptions) -> Self {
        self.protection.frame_options = Some(frame_options);
        self
    }

    #[must_use]
    pub const fn in_group(mut self, group: RouteGroup) -> Self {
        self.group = group;
        self
    }

}

#[must_use]
pub const fn gateway_protection(scope: &'static str, budget: RateBudget) -> RouteProtection {
    RouteProtection::new(
        AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential),
        ContextKind::HandlerResolved,
    )
    .with_rate_limit(RateLimit::new(scope, budget).with_bearer_window())
}

#[must_use]
pub fn path_under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}
