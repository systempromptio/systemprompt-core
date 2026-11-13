//! Mounts for the route groups that sit behind a cargo feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;

use super::{RouteMountError, protocol};

#[cfg_attr(
    not(any(
        feature = "agent-runs",
        feature = "bridge",
        feature = "slack",
        feature = "teams"
    )),
    expect(
        unused_mut,
        unused_variables,
        clippy::unnecessary_wraps,
        clippy::missing_const_for_fn,
        reason = "every route group mounted here is behind a cargo feature"
    )
)]
pub(super) fn mount_feature_routes(
    mut router: Router,
    mount: &protocol::MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let serves = mount.serves;
    #[cfg(feature = "agent-runs")]
    if serves(systemprompt_extension::RouteGroup::Agent) {
        router = super::agent_runs::mount_agent_runs(router, mount)?;
    }
    #[cfg(feature = "bridge")]
    if serves(systemprompt_extension::RouteGroup::Managed) {
        router = super::managed::mount(router, mount)?;
    }
    #[cfg(any(feature = "slack", feature = "teams"))]
    if serves(systemprompt_extension::RouteGroup::Messaging) {
        router = protocol::mount_messaging(router, mount)?;
    }
    Ok(router)
}
