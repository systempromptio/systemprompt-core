//! Route groups a capability registers into the gateway surface.
//!
//! A capability that serves part of the bridge-facing surface (the bridge
//! inbox, say) submits a [`GatewayRouteRegistration`] with
//! [`register_gateway_route!`](crate::register_gateway_route), mirroring
//! `register_gateway_guard!`. The gateway mount builds each registration whose
//! owning extension is linked and enabled, hands its factory the
//! [`GatewayCredential`] the inference surface authenticates with, and
//! protects the result exactly as its own halves: handler-verified by the
//! gateway credential, under the registration's own rate-limit scope and
//! budget with the bearer window, merged inside the gateway access log.
//!
//! The registration chooses only its router, scope and budget. The policy,
//! the verifier, the bearer window and the access log are the gateway's, so a
//! registration cannot mount a group that is gated differently from the rest
//! of the surface. A registration whose extension is disabled in the profile
//! is not built and its paths answer 404; one whose factory cannot find its
//! state fails startup, as a gateway that cannot be built does.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::http::HeaderMap;
use systemprompt_extension::{ExtensionContext, RateBudget};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_provider_contracts::MissingDependency;

use super::bridge_error::authenticate_bridge;
use crate::kernel::{ApiHttpError, JwtContextExtractor};

/// The gateway's caller verification, handed to a registered route group.
#[derive(Clone)]
pub struct GatewayCredential(Arc<JwtContextExtractor>);

impl std::fmt::Debug for GatewayCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayCredential").finish_non_exhaustive()
    }
}

impl GatewayCredential {
    pub(crate) fn new(jwt: &Arc<JwtContextExtractor>) -> Self {
        Self(Arc::clone(jwt))
    }

    pub async fn user_id(&self, headers: &HeaderMap) -> Result<UserId, ApiHttpError> {
        let (_claims, user) = authenticate_bridge(&self.0, headers).await?;
        Ok(user.id)
    }
}

pub type GatewayRouteFactory =
    fn(&dyn ExtensionContext, GatewayCredential) -> Result<Router, MissingDependency>;

/// One capability route group served inside the gateway surface.
#[derive(Debug, Clone, Copy)]
pub struct GatewayRouteRegistration {
    pub extension: &'static str,
    pub scope: &'static str,
    pub budget: RateBudget,
    pub factory: GatewayRouteFactory,
}

systemprompt_extension::__inventory::collect!(GatewayRouteRegistration);

#[macro_export]
macro_rules! register_gateway_route {
    (
        extension = $extension:expr,
        scope = $scope:expr,
        budget = $budget:expr,
        factory = $factory:expr $(,)?
    ) => {
        $crate::kernel::__inventory::submit! {
            $crate::kernel::GatewayRouteRegistration {
                extension: $extension,
                scope: $scope,
                budget: $budget,
                factory: $factory,
            }
        }
    };
}

#[derive(Debug)]
pub(crate) struct RegisteredGatewayRoutes {
    pub(crate) scope: &'static str,
    pub(crate) budget: RateBudget,
    pub(crate) router: Router,
}

pub(crate) fn build_registered(
    ctx: &dyn ExtensionContext,
    disabled: &[ExtensionId],
    jwt: &Arc<JwtContextExtractor>,
) -> Result<Vec<RegisteredGatewayRoutes>, MissingDependency> {
    let mut built = Vec::new();
    for registration in systemprompt_extension::__inventory::iter::<GatewayRouteRegistration> {
        let enabled = ctx.has_extension(registration.extension)
            && !disabled.iter().any(|id| id == registration.extension);
        if !enabled {
            tracing::info!(
                extension = registration.extension,
                scope = registration.scope,
                reason = "extension_disabled",
                "Gateway route group not mounted"
            );
            continue;
        }
        let router = (registration.factory)(ctx, GatewayCredential::new(jwt))?;
        built.push(RegisteredGatewayRoutes {
            scope: registration.scope,
            budget: registration.budget,
            router,
        });
    }
    Ok(built)
}
