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
//! The registration names its owning extension, its path under the gateway
//! base, its scope and budget, and its router factory. The policy, the
//! verifier, the bearer window and the access log are the gateway's. Every
//! linked registration is a core declaration: it is admitted only when the
//! closed core declaration table lists its extension id and exact path with
//! that scope and budget (`admit_gateway_route`), registrations may not
//! overlap one another (`refuse_overlap`) or a built-in gateway prefix
//! (`builtin::builtin_gateway_prefixes`), and anything else refuses boot
//! with a typed `MountError`, whether or not its extension is enabled. The
//! built router answers only request paths under its own path. A
//! registration whose extension is disabled in the profile is not built and
//! its paths answer 404; one whose factory cannot find its state fails
//! startup, as a gateway that cannot be built does.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::http::HeaderMap;
use systemprompt_extension::{ExtensionContext, RateBudget};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_models::modules::ApiPaths;
use systemprompt_provider_contracts::MissingDependency;

use super::bridge_error::authenticate_bridge;
use super::builtin::{builtin_gateway_prefixes, confined};
use crate::kernel::{ApiHttpError, JwtContextExtractor};
use crate::services::middleware::{MountError, admit_gateway_route, path_under, refuse_overlap};

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
    pub path: &'static str,
    pub scope: &'static str,
    pub budget: RateBudget,
    pub factory: GatewayRouteFactory,
}

systemprompt_extension::__inventory::collect!(GatewayRouteRegistration);

#[macro_export]
macro_rules! register_gateway_route {
    (
        extension = $extension:expr,
        path = $path:expr,
        scope = $scope:expr,
        budget = $budget:expr,
        factory = $factory:expr $(,)?
    ) => {
        $crate::kernel::__inventory::submit! {
            $crate::kernel::GatewayRouteRegistration {
                extension: $extension,
                path: $path,
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

/// Why a gateway route registration could not be built.
#[derive(Debug, thiserror::Error)]
pub enum GatewayRouteError {
    #[error(transparent)]
    Refused(#[from] MountError),

    #[error(transparent)]
    MissingState(#[from] MissingDependency),
}

pub fn admit_registrations(
    linked: &[GatewayRouteRegistration],
    builtin: &[&'static str],
) -> Result<(), MountError> {
    for registration in linked {
        admit_gateway_route(
            registration.extension,
            registration.path,
            registration.scope,
            registration.budget,
        )?;
        let relative = ApiPaths::gateway_relative(registration.path);
        if let Some(shadowed) = builtin
            .iter()
            .find(|prefix| path_under(relative, prefix) || path_under(prefix, relative))
        {
            return Err(MountError::GatewayRouteShadowsBuiltin {
                path: registration.path,
                builtin: *shadowed,
            });
        }
    }
    let prefixes: Vec<&'static str> = linked.iter().map(|r| r.path).collect();
    refuse_overlap(&prefixes)
}

pub(crate) fn build_registered(
    ctx: &dyn ExtensionContext,
    disabled: &[ExtensionId],
    jwt: &Arc<JwtContextExtractor>,
) -> Result<Vec<RegisteredGatewayRoutes>, GatewayRouteError> {
    let linked: Vec<GatewayRouteRegistration> =
        systemprompt_extension::__inventory::iter::<GatewayRouteRegistration>
            .into_iter()
            .copied()
            .collect();
    admit_registrations(&linked, &builtin_gateway_prefixes())?;
    let mut built = Vec::new();
    for registration in linked {
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
        let router = confined(
            (registration.factory)(ctx, GatewayCredential::new(jwt))?,
            vec![ApiPaths::gateway_relative(registration.path)],
        );
        built.push(RegisteredGatewayRoutes {
            scope: registration.scope,
            budget: registration.budget,
            router,
        });
    }
    Ok(built)
}
