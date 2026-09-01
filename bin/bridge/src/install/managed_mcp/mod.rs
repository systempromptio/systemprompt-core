//! Claude Code CLI enterprise MCP policy: `managed-mcp.json` plus the
//! `allowedMcpServers` allowlist in `managed-settings.json`.
//!
//! This is a different surface from [`crate::install::mdm`], which writes the
//! Claude Desktop `managedMcpServers` policy value (an array, under a registry
//! key or plist). The CLI reads a standalone JSON file at a fixed system path
//! and, once it exists, loads *only* the servers it names — plugin-provided
//! servers and claude.ai connectors are suppressed.
//!
//! Both files live in a system directory and need elevation to write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod render;
mod write;

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use render::{read_settings, render_managed_mcp, render_managed_settings, render_pretty};
use write::{WriteOutcome, body_matches, clear_direct, write_both};

const MANAGED_MCP_FILE: &str = "managed-mcp.json";
const MANAGED_SETTINGS_FILE: &str = "managed-settings.json";

#[must_use]
pub(crate) fn policy_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support/ClaudeCode")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\Program Files\ClaudeCode")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from("/etc/claude-code")
    }
}

pub(crate) fn server_map(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
) -> Result<Map<String, Value>, std::io::Error> {
    let bearer = loopback.bearer()?;
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    let mut map = Map::new();
    for slug in slugs {
        map.insert(
            slug.clone(),
            json!({
                "type": "http",
                "url": loopback.mcp_url(slug.as_str()),
                "headers": { "Authorization": bearer.clone() },
            }),
        );
    }
    Ok(map)
}

// Why: on Enforced the caller must not write per-user MCP config
// (`managed-mcp.json` suppresses it); on Unenforced it must, or MCP is absent.
// Declined = user cancelled the elevation prompt — treat like Unenforced for
// the caller but logged distinctly so operators can see the intent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyOutcome {
    Enforced,
    Unenforced,
    Declined,
}

pub(crate) fn apply_policy(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
    allow_claude_ai_connectors: bool,
) -> PolicyOutcome {
    let dir = policy_dir();
    let servers = match server_map(loopback, registry) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %e,
                "loopback secret unavailable; leaving the existing MCP policy in place"
            );
            return PolicyOutcome::Unenforced;
        },
    };

    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let mcp_body = match render_managed_mcp(&servers) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                path = %mcp_path.display(),
                error = %e,
                "failed to render managed-mcp.json body",
            );
            return PolicyOutcome::Unenforced;
        },
    };

    let settings_path = dir.join(MANAGED_SETTINGS_FILE);
    let settings_body =
        match render_managed_settings(&settings_path, &servers, allow_claude_ai_connectors) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    target: "bridge::install::managed-mcp",
                    path = %settings_path.display(),
                    error = %e,
                    "failed to render managed-settings.json body",
                );
                return PolicyOutcome::Unenforced;
            },
        };

    // Why: diff-first — if both files already match, skip elevation entirely so
    // idempotent syncs don't prompt.
    if body_matches(&mcp_path, &mcp_body) && body_matches(&settings_path, &settings_body) {
        return PolicyOutcome::Enforced;
    }

    match write_both(&mcp_path, &mcp_body, &settings_path, &settings_body) {
        WriteOutcome::Ok => {
            tracing::info!(
                target: "bridge::install::managed-mcp",
                path = %mcp_path.display(),
                servers = servers.len(),
                "Claude Code MCP policy applied — the CLI now has exclusive control over MCP servers"
            );
            PolicyOutcome::Enforced
        },
        WriteOutcome::Declined => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                "user declined the administrator authorization prompt; Claude Code MCP policy \
                 was not written — per-plugin .mcp.json files remain in place"
            );
            PolicyOutcome::Declined
        },
        WriteOutcome::Failed(msg) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %msg,
                "failed to write Claude Code MCP policy — falling back to per-plugin .mcp.json"
            );
            PolicyOutcome::Unenforced
        },
    }
}

// Why: removes the files rather than writing an empty server map — an empty
// managed set leaves MCP disabled entirely instead of restoring the unmanaged
// default.
pub(crate) fn clear_policy() {
    let dir = policy_dir();
    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let settings_path = dir.join(MANAGED_SETTINGS_FILE);

    // Why: try the direct removal first — a privileged user, or files that
    // never existed, must not trigger an elevation prompt.
    let stripped_settings_body = if settings_path.exists() {
        read_settings(&settings_path)
            .and_then(|mut doc| {
                doc.remove("allowedMcpServers");
                doc.remove("allowManagedMcpServersOnly");
                doc.remove("allowAllClaudeAiMcps");
                render_pretty(&Value::Object(doc))
            })
            .ok()
    } else {
        None
    };

    let mcp_exists = mcp_path.exists();
    let direct_ok = clear_direct(&mcp_path, &settings_path, stripped_settings_body.as_deref());
    if direct_ok || (!mcp_exists && stripped_settings_body.is_none()) {
        return;
    }

    #[cfg(target_os = "macos")]
    write::clear_elevated(&mcp_path, &settings_path, stripped_settings_body.as_deref());
    #[cfg(not(target_os = "macos"))]
    tracing::warn!(
        target: "bridge::install::managed-mcp",
        path = %mcp_path.display(),
        "could not remove the Claude Code MCP policy — administrator privileges required"
    );
}
