//! Mounts for the route groups that sit behind a cargo feature: the managed
//! consumer routes of the `bridge` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::router::DynConsumerCatalog;

use super::{RouteMountError, protocol};

pub(super) fn mount_feature_routes(
    router: Router,
    mount: &protocol::MountCtx<'_>,
    consumer_catalog: Option<DynConsumerCatalog>,
) -> Result<Router, RouteMountError> {
    match consumer_catalog {
        Some(catalog) if (mount.serves)(systemprompt_extension::RouteGroup::Managed) => {
            super::managed::mount(router, mount, catalog)
        },
        _ => Ok(router),
    }
}
