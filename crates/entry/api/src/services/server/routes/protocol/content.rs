//! The content-and-misc route group the server mounts itself: the services
//! tree sync routes the `cloud` member's `SyncSurface` builds (absent when
//! `cloud` is not admitted) and the admin routes (CLI gateway, services bundle). The
//! caller's session revocation and API-key management are the
//! `identity-http` member's. The content routes
//! and the short-link redirect are declared by the `content-http` extension;
//! the analytics and engagement routes by the `analytics-http` extension; the
//! marketplace routes by the `marketplace-http` extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::DynSyncSurface;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::UserType;
use systemprompt_models::modules::ApiPaths;

use super::{MountCtx, declare};
use crate::services::middleware::MountError;
use crate::services::server::routes::RouteMountError;

pub(crate) fn mount_content_and_misc(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let router = mount_sync(router, mount)?;
    Ok(mount_accounts(router, mount)?)
}

fn mount_sync(router: Router, mount: &MountCtx<'_>) -> Result<Router, RouteMountError> {
    let owner = ExtensionId::new(CoreId::Cloud.id());
    let kernel = mount.composed.kernel;
    let (Some(standing), Some(surface)) = (
        mount.composed.admitted.built().standing_of(&owner),
        kernel.state().find::<DynSyncSurface>(),
    ) else {
        return Ok(router);
    };
    let sync = surface
        .build(&kernel.route_scoped(&owner, standing, &[]))
        .map_err(|source| RouteMountError::initialization("sync", source))?;
    Ok(mount.mount(
        router,
        declare(
            sync,
            ApiPaths::SYNC_BASE,
            AuthzPolicy::restricted_to(&[UserType::Service]),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        ),
    )?)
}

fn mount_accounts(router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::ContentAndMisc;

    mount.mount(
        router,
        declare(
            crate::routes::admin::router().with_state(ctx.clone()),
            ApiPaths::ADMIN_BASE,
            AuthzPolicy::admin(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("admin", RateBudget::PerSecond(10))),
    )
}
