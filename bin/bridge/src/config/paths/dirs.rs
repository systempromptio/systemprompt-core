//! Cowork session and bridge working-directory locations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

#[must_use]
pub fn cowork3p_sessions_root() -> Option<PathBuf> {
    cowork3p_base().map(|base| base.join("Claude-3p").join("local-agent-mode-sessions"))
}

fn cowork3p_base() -> Option<PathBuf> {
    if let Some(base) = crate::basedirs::config_home_override() {
        return Some(base);
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        crate::basedirs::home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    // Why: Cowork ships macOS and Windows builds only, so an XDG-style Linux path
    // would name a directory no install can ever create.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

pub const COWORK_PLUGINS_SUBDIR: &str = "cowork_plugins";

pub const COWORK_ARTIFACTS_SUBDIR: &str = "cowork_artifacts";

pub const WORKSPACE_ARTIFACTS_SUBDIR: &str = "systemprompt/artifacts";

// Why: the pre-trusted Cowork workspace named by `allowedWorkspaceFolders` is
// a connected folder, so a file the bridge stages there is a path Cowork's
// `create_artifact` accepts — no shell copy into the session is needed.
#[must_use]
pub fn workspace_dir() -> Option<PathBuf> {
    let name = crate::brand::brand().workspace_dir_name;
    if name.is_empty() {
        return None;
    }
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(crate::basedirs::home_dir)?;
    Some(home.join(name))
}

#[must_use]
pub fn workspace_artifacts_dir() -> Option<PathBuf> {
    workspace_dir().map(|w| w.join(WORKSPACE_ARTIFACTS_SUBDIR))
}

#[must_use]
pub fn claude_code_policy_dir() -> PathBuf {
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

#[must_use]
pub fn bridge_working_dir() -> Option<PathBuf> {
    bridge_state_base().map(|base| base.join(crate::brand::brand().working_dir_name))
}

fn bridge_state_base() -> Option<PathBuf> {
    if let Some(base) = crate::basedirs::state_home_override() {
        return Some(base);
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        crate::basedirs::home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        crate::basedirs::home_dir().map(|h| h.join(".local").join("state"))
    }
}

#[must_use]
pub fn bridge_staging_dir() -> Option<PathBuf> {
    bridge_working_dir().map(|p| p.join("staging"))
}

#[must_use]
pub fn bridge_metadata_dir() -> Option<PathBuf> {
    bridge_working_dir().map(|p| p.join("metadata"))
}

#[must_use]
pub fn claude_cli_home() -> Option<PathBuf> {
    crate::basedirs::home_dir().map(|h| h.join(".claude"))
}

#[must_use]
pub fn claude_cli_plugins_dir() -> Option<PathBuf> {
    claude_cli_home().map(|h| h.join("plugins"))
}

#[must_use]
pub fn claude_cli_settings_path() -> Option<PathBuf> {
    claude_cli_home().map(|h| h.join("settings.json"))
}
