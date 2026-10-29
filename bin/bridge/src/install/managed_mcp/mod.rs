//! Claude Code CLI enterprise MCP policy: the bridge never writes it, and
//! removes only a policy an earlier bridge wrote; a policy anyone else
//! deployed is left in place and reported.
//!
//! `managed-mcp.json` puts the CLI into exclusive mode (plugin and user
//! servers vanish) and `allowManagedMcpServersOnly` in `managed-settings.json`
//! denies every server not on the allowlist — including Cowork's built-in
//! workspace server, so the sandbox bash tool and every skill that needs it
//! stop working. Servers reach the CLI through per-plugin `.mcp.json` files
//! instead, and Claude tools are never blocked by policy.
//!
//! Both files live in a system directory and need elevation to remove.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod write;

use std::path::PathBuf;

pub(crate) use crate::claude_policy::{
    MANAGED_MCP_FILE, MANAGED_SETTINGS_FILE, PolicyAuthor, holds_foreign_mcp_keys,
    managed_mcp_author, stripped_settings,
};

pub(crate) fn policy_dir() -> PathBuf {
    crate::config::paths::claude_code_policy_dir()
}

pub(crate) fn clear_policy(gate: &crate::elevation_gate::ElevationGate) -> std::io::Result<()> {
    let dir = policy_dir();
    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let settings_path = dir.join(MANAGED_SETTINGS_FILE);
    let owned_mcp = match managed_mcp_author(&mcp_path)? {
        Some(PolicyAuthor::Bridge) => Some(mcp_path.as_path()),
        Some(PolicyAuthor::Foreign) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                path = %mcp_path.display(),
                "a Claude Code MCP policy the bridge did not write is in place; left untouched"
            );
            None
        },
        None => None,
    };
    if holds_foreign_mcp_keys(&settings_path) {
        tracing::warn!(
            target: "bridge::install::managed-mcp",
            path = %settings_path.display(),
            "Claude Code MCP allowlist keys the bridge did not write are in place; left untouched"
        );
    }
    let stripped = stripped_settings(&settings_path)?;
    if owned_mcp.is_none() && stripped.is_none() {
        return Ok(());
    }
    match write::clear_direct(owned_mcp, &settings_path, stripped.as_deref()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            write::clear_elevated(gate, owned_mcp, &settings_path, stripped.as_deref())
        },
        Err(e) => Err(std::io::Error::new(
            e.kind(),
            format!("{}: {e}", dir.display()),
        )),
    }
}
