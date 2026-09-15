//! Evaluation administration and separately credentialed worker transport.
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
        crate::routes::evaluation::campaigns::router()
            .layer(axum::middleware::from_fn_with_state(
                mount.ctx.clone(),
                crate::routes::evaluation::optimization_origin::protect,
            ))
            .with_state(
                crate::routes::evaluation::optimization_state::OptimizationState::new(
                    mount.ctx.clone(),
                ),
            )
            .with_rate_limit(mount.limits, 10, "admin")?
            .with_auth(mount.user_middleware.clone(), AuthzPolicy::admin())
            .layer(axum::middleware::from_fn(
                crate::routes::evaluation::contract::normalize,
            )),
    );
    let router = router.nest(
        "/api/v1",
        crate::routes::evaluation::consumer::router()
            .layer(axum::middleware::from_fn_with_state(
                mount.ctx.clone(),
                crate::routes::evaluation::optimization_origin::protect,
            ))
            .with_state(mount.ctx.clone())
            .with_rate_limit(mount.limits, 10, "consumer_evidence")?
            .layer(axum::middleware::from_fn(
                crate::routes::evaluation::contract::normalize,
            )),
    );
    mount_worker(router, mount)
}

// Why: workers present environment-scoped execution credentials rather than
// user JWTs. Each handler authenticates the credential against its lease fence.
fn mount_worker(router: Router, mount: &MountCtx<'_>) -> Result<Router, LoaderError> {
    let evaluator = crate::routes::evaluation::router_from_context(mount.ctx).map_err(|error| {
        LoaderError::InitializationFailed {
            extension: "evaluation-worker".to_owned(),
            message: error.to_string(),
        }
    })?;
    Ok(router.nest(
        "/api/v1/evaluation/worker",
        evaluator.with_rate_limit(mount.limits, 10, "evaluation_worker")?,
    ))
}
