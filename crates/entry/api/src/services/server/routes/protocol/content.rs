//! The content-and-misc route group: content and redirects, the platform
//! routes (sync, marketplace, analytics, engagement), and the account routes
//! (users, admin).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup, RouteProtection,
};
use systemprompt_models::auth::UserType;
use systemprompt_models::modules::ApiPaths;

use super::{MountCtx, declare};
use crate::services::middleware::{MountError, place};

pub(crate) fn mount_content_and_misc(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let router = mount_content(router, mount)?;
    let router = mount_platform(router, mount)?;
    mount_accounts(router, mount)
}

const CONTENT_RATE: RateLimit = RateLimit::new("content", RateBudget::Content);

fn mount_content(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::ContentAndMisc;
    let content_rate = CONTENT_RATE;

    let public_content = mount.protect(
        crate::routes::content::public_router(ctx),
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
            .with_rate_limit(content_rate),
        ApiPaths::CONTENT_BASE,
    )?;
    let authenticated_content = mount.protect(
        crate::routes::content::authenticated_router(ctx),
        &RouteProtection::new(AuthzPolicy::user(), ContextKind::User).with_rate_limit(content_rate),
        ApiPaths::CONTENT_BASE,
    )?;
    router = place(
        router,
        ApiPaths::CONTENT_BASE,
        public_content.merge(authenticated_content),
    );

    router = mount.mount(
        router,
        declare(
            crate::routes::content::redirect_router(ctx.content_repositories()),
            systemprompt_extension::registry::WEB_ROOT_BASE_PATH,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(content_rate),
    )?;

    Ok(router)
}

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
