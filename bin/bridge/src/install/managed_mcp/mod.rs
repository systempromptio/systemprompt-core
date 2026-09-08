//! Claude Code CLI enterprise MCP policy: the bridge never writes it, and
//! removes any it wrote before.
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

pub(crate) use crate::claude_policy::{MANAGED_MCP_FILE, MANAGED_SETTINGS_FILE, stripped_settings};

pub(crate) fn policy_dir() -> PathBuf {
    crate::config::paths::claude_code_policy_dir()
}

// Why: Claude Code treats an empty managed MCP file as exclusive mode with no
// servers.
pub(crate) fn clear_policy() -> std::io::Result<()> {
    let dir = policy_dir();
    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let settings_path = dir.join(MANAGED_SETTINGS_FILE);
    let stripped = stripped_settings(&settings_path)?;
    match write::clear_direct(&mcp_path, &settings_path, stripped.as_deref()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            write::clear_elevated(&mcp_path, &settings_path, stripped.as_deref())
        },
        Err(e) => Err(std::io::Error::other(format!("{}: {e}", dir.display()))),
    }
}
