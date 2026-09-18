//! Claude Code `permissions` rules for the managed MCP servers, applied and
//! cleared alongside the mirrored org plugins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::host_sync::{ApplyError, HostSyncCtx};
use crate::install::mdm::claude_code_settings::permissions;

// Why: the manifest carries the decision for every managed server's tools
// (allow by default); Claude Code only stops prompting once that decision is
// in its `permissions` rules, so the rules are re-derived on every sync.
pub(super) fn apply_tool_permissions(ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
    let rules = permissions::rules_for(ctx.manifest, ctx.plugin_mcp_servers);
    let outcome = permissions::apply_permissions(&rules).map_err(|e| ApplyError::Io {
        context: "claude code tool permissions".to_owned(),
        source: std::io::Error::other(e.to_string()),
    })?;
    match outcome {
        permissions::PermissionOutcome::Written(lines) => {
            for line in lines {
                tracing::info!(target: "bridge::claude-code-cli", detail = %line, "tool permissions");
            }
        },
        permissions::PermissionOutcome::NoCarrier { rules, standalone } => {
            ctx.warnings.push(
                crate::host_sync::HostWarningKind::PermissionRules,
                "claude-code",
                format!(
                    "{rules} tool permission rules not applied: no Claude Code settings file is \
                     managed by this bridge; run `install --apply` to create {}",
                    standalone.display()
                ),
            );
        },
    }
    Ok(())
}

pub(super) fn clear_tool_permissions() -> Result<(), ApplyError> {
    permissions::apply_permissions(&permissions::PermissionRules::default())
        .map_err(|e| ApplyError::Io {
            context: "clear claude code tool permissions".to_owned(),
            source: std::io::Error::other(e.to_string()),
        })
        .map(|_| ())
}
