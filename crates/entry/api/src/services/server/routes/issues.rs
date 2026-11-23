//! Mounts `/api/v1/issues` behind the user authz gate and the content rate
//! limit. Triage authorization is the routes' own check on the triage roles
//! or admin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::issues::{ISSUES_PATH, IssuesState, router};

pub(super) fn mount_issues(
    router_in: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let state = IssuesState::from_context(mount.ctx)
        .map_err(|source| RouteMountError::initialization("issues", source))?;
    Ok(mount.mount(
        router_in,
        declare(
            router(state),
            ISSUES_PATH,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(RateLimit::new("issues", RateBudget::Content)),
    )?)
}
