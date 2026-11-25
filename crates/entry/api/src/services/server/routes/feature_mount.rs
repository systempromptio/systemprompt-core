//! Mounts for the route groups that sit behind a cargo feature: the managed
//! consumer routes of the `bridge` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;

use super::{RouteMountError, protocol};

pub(super) fn mount_feature_routes(
    router: Router,
    mount: &protocol::MountCtx<'_>,
    managed_consumer: Option<systemprompt_gateway::surface::ManagedConsumer>,
) -> Result<Router, RouteMountError> {
    match managed_consumer {
        Some(managed_consumer) if (mount.serves)(systemprompt_extension::RouteGroup::Managed) => {
            super::managed::mount(router, mount, managed_consumer)
        },
        _ => Ok(router),
    }
}
