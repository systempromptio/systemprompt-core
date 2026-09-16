//! Managed-resource administration and the device-credential consumer surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::LoaderError;

use super::protocol::MountCtx;
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

pub(super) fn mount(router: Router, mount: &MountCtx<'_>) -> Result<Router, LoaderError> {
    let router = router.nest(
        "/api/v1",
        crate::routes::managed::router()
            .layer(axum::middleware::from_fn_with_state(
                mount.ctx.clone(),
                crate::routes::managed::origin::protect,
            ))
            .with_state(crate::routes::managed::state::ManagedState::new(
                mount.ctx.clone(),
            ))
            .with_rate_limit(mount.limits, 10, "admin")?
            .with_auth(mount.user_middleware.clone(), AuthzPolicy::admin())
            .layer(axum::middleware::from_fn(
                crate::routes::managed::contract::normalize,
            )),
    );
    Ok(router.nest(
        "/api/v1",
        crate::routes::managed::consumer::router()
            .layer(axum::middleware::from_fn_with_state(
                mount.ctx.clone(),
                crate::routes::managed::origin::protect,
            ))
            .with_state(mount.ctx.clone())
            .with_rate_limit(mount.limits, 10, "consumer_evidence")?
            .layer(axum::middleware::from_fn(
                crate::routes::managed::contract::normalize,
            )),
    ))
}
