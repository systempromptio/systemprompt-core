//! The content-and-misc route group the server mounts itself: the services
//! tree sync route and the admin routes (CLI gateway, services bundle, and the
//! `identity-http` member's API-key management nested at `/api-keys` when the
//! member is admitted). One admin gate and one per-caller `admin` throttle
//! cover the whole `/api/v1/admin` router. The content routes
//! and the short-link redirect are declared by the `content-http` extension;
//! the analytics and engagement routes by the `analytics-http` extension; the
//! marketplace routes by the `marketplace-http` extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::auth::UserType;
use systemprompt_models::modules::ApiPaths;

use super::{MountCtx, declare};
use crate::services::middleware::MountError;

pub(crate) fn mount_content_and_misc(
    router: Router,
    mount: &MountCtx<'_>,
    api_keys: Option<Router>,
) -> Result<Router, MountError> {
    let router = mount_platform(router, mount)?;
    mount_admin(router, mount, api_keys)
}

fn mount_platform(router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    mount.mount(
        router,
        declare(
            crate::routes::sync::router().with_state(mount.ctx.clone()),
            ApiPaths::SYNC_BASE,
            AuthzPolicy::restricted_to(&[UserType::Service]),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        ),
    )
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
