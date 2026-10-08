//! Claude Code settings re-derived from the manifest on every sync.
//!
//! The `permissions` rules for the managed MCP servers and the skill listing
//! budget the marketplaces declare, applied and cleared alongside the
//! mirrored org plugins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::host_sync::{ApplyError, HostSyncCtx, HostSyncReport};
use crate::install::mdm::claude_code_settings::{permissions, skill_budget};

// Why: the manifest carries the decision for every managed server's tools
// (allow by default); Claude Code only stops prompting once that decision is
// in its `permissions` rules, so the rules are re-derived on every sync.
fn apply_tool_permissions(ctx: &HostSyncCtx<'_>) -> Result<HostSyncReport, ApplyError> {
    let mut report = HostSyncReport::ok();
    let rules = permissions::rules_for(ctx.manifest, ctx.plugin_mcp_servers);
    let outcome = permissions::apply_permissions(&rules).map_err(|e| ApplyError::Step {
        context: "claude code tool permissions",
        source: Box::new(e),
    })?;
    match outcome {
        permissions::PermissionOutcome::Written(lines) => {
            for line in lines {
                tracing::info!(target: "bridge::claude-code-cli", detail = %line, "tool permissions");
            }
        },
        permissions::PermissionOutcome::NoCarrier { rules, standalone } => {
            report.warn(
                crate::host_sync::HostWarningKind::PermissionRules,
                systemprompt_models::bridge::host::HostKind::ClaudeCode,
                format!(
                    "{rules} tool permission rules not applied: no Claude Code settings file is \
                     managed by this bridge; run `install --apply` to create {}",
                    standalone.display()
                ),
            );
        },
    }
    Ok(report)
}

fn clear_tool_permissions() -> Result<(), ApplyError> {
    permissions::apply_permissions(&permissions::PermissionRules::default())
        .map_err(|e| ApplyError::Step {
            context: "clear claude code tool permissions",
            source: Box::new(e),
        })
        .map(|_| ())
}

// Why: Claude Code truncates skill descriptions past its listing budget
// (8,000 characters by default) and then stops loading those skills on its
// own, so the budget the marketplaces declare is re-applied on every sync.
fn apply_skill_listing_budget(budget: Option<u32>) -> Result<(), ApplyError> {
    let lines = skill_budget::apply_skill_budget(budget).map_err(|e| ApplyError::Step {
        context: "claude code skill listing budget",
        source: Box::new(e),
    })?;
    for line in lines {
        tracing::info!(target: "bridge::claude-code-cli", detail = %line, "skill listing budget");
    }
    Ok(())
}

pub(super) fn apply_client_settings(ctx: &HostSyncCtx<'_>) -> Result<HostSyncReport, ApplyError> {
    let report = apply_tool_permissions(ctx)?;
    apply_skill_listing_budget(skill_budget::budget_for(ctx.manifest))?;
    Ok(report)
}

pub(super) fn clear_client_settings() -> Result<(), ApplyError> {
    clear_tool_permissions()?;
    apply_skill_listing_budget(None)
}
