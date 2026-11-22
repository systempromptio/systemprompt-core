//! Mounts `/api/v1/brain` (user policy) and `/api/v1/admin/brain` (admin
//! policy) as two declarations behind the user context and the brain rate
//! limit. An enabled brain without a user provider fails the mount rather
//! than serving routes that cannot resolve their callers; with no brain the
//! routers answer 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::AppContext as AppContextTrait;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};

pub(super) fn mount_brain_routes(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let brain_routes = match ctx.brain() {
        Some(brain) => {
            let users = super::require_provider("brain", ctx.user_provider())?;
            let (service, recall, profile) = brain.routes();
            Some(
                crate::routes::brain::BrainRoutes::new(service, recall, profile, users)
                    .with_subject_providers(ctx.subject_providers().clone()),
            )
        },
        None => None,
    };
    let brain_rate = RateLimit::new("brain", RateBudget::Brain);
    router = mount.mount(
        router,
        declare(
            crate::routes::brain::router(brain_routes.clone()),
            ApiPaths::BRAIN,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(brain_rate),
    )?;
    Ok(mount.mount(
        router,
        declare(
            crate::routes::brain::admin_router(brain_routes),
            ApiPaths::ADMIN_BRAIN,
            AuthzPolicy::admin(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(brain_rate),
    )?)
}
