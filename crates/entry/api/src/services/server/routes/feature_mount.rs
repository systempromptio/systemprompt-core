//! Mounts for the route groups that sit behind a cargo feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;

use super::{RouteMountError, protocol};

#[cfg_attr(
    not(feature = "bridge"),
    expect(
        unused_mut,
        unused_variables,
        clippy::unnecessary_wraps,
        reason = "every route group mounted here is behind a cargo feature"
    )
)]
pub(super) fn mount_feature_routes(
    mut router: Router,
    mount: &protocol::MountCtx<'_>,
    managed_consumer: Option<systemprompt_gateway::surface::ManagedConsumer>,
) -> Result<Router, RouteMountError> {
    let serves = mount.serves;
    #[cfg(feature = "bridge")]
    if serves(systemprompt_extension::RouteGroup::Managed)
        && let Some(managed_consumer) = managed_consumer
    {
        router = super::managed::mount(router, mount, managed_consumer)?;
    }
    Ok(router)
}
