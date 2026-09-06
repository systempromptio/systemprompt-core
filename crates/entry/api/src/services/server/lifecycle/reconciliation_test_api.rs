//! Test seams over this module's private reconciliation helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_runtime::AppContext;

#[must_use]
pub fn service_row_is_stale(status: &str, pid: Option<i32>, name_key: &str, name: &str) -> bool {
    super::service_row_is_stale(status, pid, name_key, name)
}

pub async fn cleanup_stale_service_entries(ctx: &AppContext) -> Result<u64> {
    super::cleanup_stale_service_entries(ctx, None).await
}

pub async fn verify_database_registration(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    ctx: &AppContext,
) -> Result<()> {
    super::verify_database_registration(required_servers, ctx, None).await
}

pub async fn handle_missing_servers(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    ctx: &AppContext,
) -> Result<()> {
    let orchestrator = std::sync::Arc::new(systemprompt_mcp::services::McpOrchestrator::new(
        std::sync::Arc::clone(ctx.db_pool()),
        (**ctx.service_repository()).clone(),
        std::sync::Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )?);
    super::handle_missing_servers(required_servers, &orchestrator, None).await
}
