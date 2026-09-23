//! The device-credential consumer surface the bridge reports installation
//! evidence through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::LoaderError;

use super::protocol::MountCtx;
use crate::services::middleware::RouterExt;

pub(super) fn mount(router: Router, mount: &MountCtx<'_>) -> Result<Router, LoaderError> {
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
