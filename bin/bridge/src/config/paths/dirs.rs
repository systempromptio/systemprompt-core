//! Cowork session and bridge working-directory locations.
//!
//! `CLAUDE_MSIX_FAMILY` is the package family of the Store (MSIX) build of
//! Claude Desktop; its `%LOCALAPPDATA%` is virtualised under
//! `Packages\<family>\LocalCache`, so that root is probed alongside the
//! plain one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

pub const CLAUDE_MSIX_FAMILY: &str = "Claude_pzs8sxrjxfjjc";

#[must_use]
pub fn cowork3p_sessions_root() -> Option<PathBuf> {
    let candidates = cowork3p_bases();
    let roots = candidates
        .iter()
        .map(|base| base.join("Claude-3p").join("local-agent-mode-sessions"));
    // Why: the MSIX build writes its data under the package's LocalCache; an
    // installed-but-never-opened Cowork has neither. Prefer whichever root
    // exists and fall back to the classic one so callers can still name it.
    roots
        .clone()
        .find(|root| root.is_dir())
        .or_else(|| roots.into_iter().next())
}

fn cowork3p_bases() -> Vec<PathBuf> {
    if let Some(base) = crate::basedirs::config_home_override() {
        return vec![base];
    }
    #[cfg(target_os = "windows")]
    {
        let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
            return Vec::new();
        };
        let msix = local
            .join("Packages")
            .join(CLAUDE_MSIX_FAMILY)
            .join("LocalCache")
            .join("Local");
        vec![local, msix]
    }
    #[cfg(target_os = "macos")]
    {
        crate::basedirs::home_dir()
            .map(|h| h.join("Library").join("Application Support"))
            .into_iter()
            .collect()
    }
    // Why: Cowork has no Linux desktop build.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Vec::new()
    }
}

pub const COWORK_PLUGINS_SUBDIR: &str = "cowork_plugins";

pub const COWORK_ARTIFACTS_SUBDIR: &str = "cowork_artifacts";

pub const WORKSPACE_ARTIFACTS_SUBDIR: &str = "systemprompt/artifacts";

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
