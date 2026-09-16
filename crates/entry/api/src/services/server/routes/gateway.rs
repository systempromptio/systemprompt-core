//! Gateway route mount: the inference router plus its public session routes,
//! both present only when the gateway is enabled for this profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::LoaderError;
use systemprompt_models::modules::ApiPaths;

use super::protocol::MountCtx;
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

pub(super) fn mount_gateway(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, LoaderError> {
    let ctx = mount.ctx;
    let gateway = crate::routes::gateway::gateway_router(ctx).map_err(|error| {
        LoaderError::InitializationFailed {
            extension: "gateway".to_owned(),
            message: error.to_string(),
        }
    })?;
    if let Some(gateway) = gateway {
        router = router.nest(
            ApiPaths::GATEWAY_BASE,
            gateway.with_rate_limit(
                mount.limits,
                ctx.config().rate_limits.gateway_per_second,
                "gateway",
            )?,
        );
        router = router.nest(
            ApiPaths::GATEWAY_PUBLIC_BASE,
            crate::routes::gateway::sessions::public_router(ctx)
                .with_rate_limit(
                    mount.limits,
                    ctx.config().rate_limits.oauth_public_per_second,
                    "oauth_public",
                )?
                .with_auth(*mount.public_middleware, AuthzPolicy::public()),
        );
    }
    Ok(router)
}
