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

mod render;
mod write;

use std::path::PathBuf;

pub use render::stripped_settings;

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

// Why: removes the files rather than writing an empty server map — an empty
// managed set leaves MCP disabled entirely instead of restoring the unmanaged
// default.
pub(crate) fn clear_policy() {
    let dir = policy_dir();
    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let settings_path = dir.join(MANAGED_SETTINGS_FILE);

    let stripped = match stripped_settings(&settings_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                path = %settings_path.display(),
                error = %e,
                "could not read managed-settings.json; leaving it in place"
            );
            None
        },
    };
    let mcp_exists = mcp_path.exists();
    if !mcp_exists && stripped.is_none() {
        return;
    }
    // Why: try the direct removal first — a privileged user must not be
    // prompted at all.
    if write::clear_direct(&mcp_path, &settings_path, stripped.as_deref()) {
        tracing::info!(
            target: "bridge::install::managed-mcp",
            "Claude Code MCP policy removed; plugin and user MCP servers are no longer shadowed"
        );
        return;
    }
    write::clear_elevated(&mcp_path, &settings_path, stripped.as_deref());
}
