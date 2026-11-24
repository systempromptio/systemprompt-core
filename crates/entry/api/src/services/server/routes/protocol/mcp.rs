//! Mounts the MCP executions lookup the `mcp-http` member's `McpSurface`
//! builds and the MCP proxy under `/api/v1/mcp`.
//!
//! Both are served only when the `mcp-http` member is admitted and the `mcp`
//! member provides the artifact ingest every proxied tool call is audited
//! through; otherwise neither is mounted (404). The executions lookup is
//! authenticated with a user context, the proxy defers to its handler (the
//! OAuth challenge) with the MCP context, and both share the `mcp` rate
//! scope and budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::DynMcpSurface;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteProtection};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;

use super::MountCtx;
use crate::services::middleware::{MountError, place};

pub(in crate::services::server::routes) fn mount_mcp(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let owner = ExtensionId::new(CoreId::McpHttp.id());
    let kernel = mount.composed.kernel;
    let (Some(standing), Some(surface), Some(_ingest)) = (
        mount.composed.admitted.built().standing_of(&owner),
        kernel.state().find::<DynMcpSurface>(),
        kernel
            .state()
            .find::<Arc<systemprompt_mcp::ArtifactIngest>>(),
    ) else {
        return Ok(router);
    };
    let built = surface.build(&kernel.route_scoped(&owner, standing, &[]))?;

    let mcp_rate = RateLimit::new("mcp", RateBudget::Mcp);
    let executions = mount.protect(
        built.executions,
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
