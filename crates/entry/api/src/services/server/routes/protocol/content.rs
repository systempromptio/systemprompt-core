//! The content-and-misc route group the server mounts itself: the services
//! tree sync routes the `cloud` member's `SyncSurface` builds (absent when
//! `cloud` is not admitted) and the admin routes (CLI gateway, services bundle,
//! and the `identity-http` member's API-key management nested at `/api-keys`
//! when the member is admitted). One admin gate and one per-caller `admin`
//! throttle cover the whole `/api/v1/admin` router. The content routes
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
    api_keys: Option<Router>,
) -> Result<Router, RouteMountError> {
    let router = mount_sync(router, mount)?;
    Ok(mount_admin(router, mount, api_keys)?)
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

fn mount_admin(
    router: Router,
    mount: &MountCtx<'_>,
    api_keys: Option<Router>,
) -> Result<Router, MountError> {
    let group = RouteGroup::ContentAndMisc;
    let admin = crate::routes::admin::router().with_state(mount.ctx.clone());
    let admin = match api_keys {
        Some(api_keys) => admin.nest(ApiPaths::admin_relative(ApiPaths::ADMIN_API_KEYS), api_keys),
        None => admin,
    };

    mount.mount(
        router,
        declare(
            admin,
            ApiPaths::ADMIN_BASE,
            AuthzPolicy::admin(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("admin", RateBudget::PerSecond(10))),
    )
}
