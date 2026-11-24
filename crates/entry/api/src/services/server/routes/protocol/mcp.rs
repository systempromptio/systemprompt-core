//! MCP route mounting: the MCP proxy group and its registry reservation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteProtection};
use systemprompt_models::modules::ApiPaths;

use super::MountCtx;
use crate::services::middleware::{MountError, place};

pub(in crate::services::server::routes) fn mount_mcp(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let ctx = mount.ctx;

    let mcp_rate = RateLimit::new("mcp", RateBudget::Mcp);
    let executions = mount.protect(
        crate::routes::proxy::mcp::executions_router(ctx),
        &RouteProtection::new(AuthzPolicy::authenticated(), ContextKind::User)
            .with_rate_limit(mcp_rate),
        ApiPaths::MCP_BASE,
    )?;
    let proxy = mount.protect(
        crate::routes::proxy::mcp::router(ctx)?,
        &RouteProtection::new(AuthzPolicy::deferred_to_handler(), ContextKind::Mcp)
            .with_rate_limit(mcp_rate),
        ApiPaths::MCP_BASE,
    )?;
    Ok(place(router, ApiPaths::MCP_BASE, executions.merge(proxy)))
}
