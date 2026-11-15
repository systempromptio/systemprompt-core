//! The content-and-misc route group the server mounts itself: the platform
//! routes (sync, marketplace, analytics, engagement) and the account routes
//! (users, admin). The content routes and the short-link redirect are
//! declared by the `content-http` extension.
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
) -> Result<Router, MountError> {
    let router = mount_platform(router, mount)?;
    mount_accounts(router, mount)
}

const CONTENT_RATE: RateLimit = RateLimit::new("content", RateBudget::Content);

fn mount_platform(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::ContentAndMisc;
    let content_rate = CONTENT_RATE;

    router = mount.mount(
        router,
        declare(
            crate::routes::sync::router().with_state(ctx.clone()),
            ApiPaths::SYNC_BASE,
            AuthzPolicy::restricted_to(&[UserType::Service]),
            ContextKind::User,
            group,
        ),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::marketplace::router().with_state(ctx.clone()),
            ApiPaths::MARKETPLACE_BASE,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        ),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::analytics::router(ctx),
            ApiPaths::ANALYTICS_BASE,
            AuthzPolicy::admin(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(content_rate),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::engagement::router(ctx),
            ApiPaths::TRACK_ENGAGEMENT,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(content_rate),
    )?;

    Ok(router)
}

fn mount_accounts(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::ContentAndMisc;

    router = mount.mount(
        router,
        declare(
            crate::routes::users::router(ctx),
            ApiPaths::CORE_USERS,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("oauth_auth", RateBudget::OauthAuth)),
    )?;

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
