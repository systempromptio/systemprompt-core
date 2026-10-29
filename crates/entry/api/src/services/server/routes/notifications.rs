//! Mounts `/api/v1/notifications` behind the user authz gate and
//! `/api/v1/admin/notifications` behind the admin gate, both under the
//! notifications rate limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::routes::notifications::{NotificationsState, admin_router, router};
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

pub(super) fn mount_notifications(
    router_in: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let state = NotificationsState::from_context(mount.ctx);
    let per_second = mount.ctx.config().rate_limits.notifications_per_second;
    Ok(router_in
        .nest(
            ApiPaths::NOTIFICATIONS,
            router(state.clone())
                .with_rate_limit(mount.limits, per_second, "notifications")?
                .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
        )
        .nest(
            ApiPaths::ADMIN_NOTIFICATIONS,
            admin_router(state)
                .with_rate_limit(mount.limits, per_second, "notifications")?
                .with_auth(mount.user_middleware.clone(), AuthzPolicy::admin()),
        ))
}
