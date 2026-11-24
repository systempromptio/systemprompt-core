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
    consumer_catalog: Option<systemprompt_marketplace::surface::DynConsumerCatalog>,
) -> Result<Router, RouteMountError> {
    let serves = mount.serves;
    #[cfg(feature = "bridge")]
    if serves(systemprompt_extension::RouteGroup::Managed)
        && let Some(catalog) = consumer_catalog
    {
        router = super::managed::mount(router, mount, catalog)?;
    }
    Ok(router)
}
