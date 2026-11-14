//! Mounts `/api/v1/notifications` behind the user authz gate and
//! `/api/v1/admin/notifications` behind the admin gate, both under the
//! notifications rate limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::protocol::{MountCtx, declare};
use crate::routes::notifications::{NotificationsState, admin_router, router};
use crate::services::middleware::MountError;

pub(super) fn mount_notifications(
    router_in: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let state = NotificationsState::from_context(mount.ctx);
    let rate_limit = RateLimit::new("notifications", RateBudget::Notifications);
    let routes = mount.mount(
        router_in,
        declare(
            router(state.clone()),
            ApiPaths::NOTIFICATIONS,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(rate_limit),
    )?;
    mount.mount(
        routes,
        declare(
            admin_router(state),
            ApiPaths::ADMIN_NOTIFICATIONS,
            AuthzPolicy::admin(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(rate_limit),
    )
}
