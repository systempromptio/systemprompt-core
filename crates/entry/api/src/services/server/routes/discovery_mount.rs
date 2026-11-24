//! Mounting the discovery and well-known routes and the provider requirement
//! they share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use axum::Router;
use systemprompt_extension::reserved::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{AuthzPolicy, ContextKind, LoaderError, RouteGroup};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::AppContext;

pub(super) fn mount_discovery(
    mut router: Router,
    mount: &MountCtx<'_>,
    identity_wellknown: Option<Router>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let serves = mount.serves;
    if serves(RouteGroup::Discovery) {
        router = mount.mount(
            router,
            declare(
                super::super::builder::discovery_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::Discovery,
            ),
        )?;
    }
    if serves(RouteGroup::Probes) {
        router = mount.mount(
            router,
            declare(
                super::super::builder::probes_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::Probes,
            ),
        )?;
    }
    if serves(RouteGroup::AuthenticatedDiscovery) {
        router = mount.mount(
            router,
            declare(
                super::super::builder::authenticated_discovery_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::authenticated(),
                ContextKind::User,
                RouteGroup::AuthenticatedDiscovery,
            ),
        )?;
    }
    if serves(RouteGroup::WellKnown) {
        router = mount.mount(
            router,
            declare(
                wellknown_router(ctx, identity_wellknown)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::WellKnown,
            ),
        )?;
    }
    Ok(router)
}

pub(super) fn require_provider<T: std::any::Any>(
    extension: &str,
    provider: Option<T>,
) -> Result<T, LoaderError> {
    provider.ok_or_else(|| LoaderError::missing_provider::<T>(ExtensionId::new(extension)))
}

pub(super) fn wellknown_router(
    ctx: &AppContext,
    identity: Option<Router>,
) -> Result<Router, LoaderError> {
    let kernel = crate::routes::wellknown_router(ctx)?;
    Ok(match identity {
        Some(identity) => identity.merge(kernel),
        None => kernel,
    })
}
