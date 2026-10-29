//! Mounts `/api/v1/issues` behind the user authz gate and the content rate
//! limit. Triage authorization is the routes' own check on the triage roles
//! or admin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::routes::issues::{ISSUES_PATH, IssuesState, router};
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

pub(super) fn mount_issues(
    router_in: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let state = IssuesState::from_context(mount.ctx);
    let per_second = mount.ctx.config().rate_limits.content_per_second;
    Ok(router_in.nest(
        ISSUES_PATH,
        router(state)
            .with_rate_limit(mount.limits, per_second, "issues")?
            .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
    ))
}
