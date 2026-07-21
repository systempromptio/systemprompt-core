//! Well-known bridge file locations and writable-directory probing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

pub const VERSION_SENTINEL: &str = "version.json";
pub const LAST_SYNC_SENTINEL: &str = "last-sync.json";
pub const USER_FRAGMENT: &str = "user.json";
pub const MCP_SERVERS_FRAGMENT: &str = "mcp-servers.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgPluginsLocation {
    pub path: PathBuf,
    pub scope: Scope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

#[cfg(target_os = "macos")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "uniform Option<PathBuf> contract across OS-cfg variants; sibling org_plugins_user \
              can legitimately return None"
)]
pub fn org_plugins_system() -> Option<PathBuf> {
    Some(PathBuf::from(
        "/Library/Application Support/Claude/org-plugins",
    ))
}

#[cfg(target_os = "macos")]
pub fn org_plugins_user() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join("Library")
            .join("Application Support")
            .join("Claude")
            .join("org-plugins")
    })
}

// Cowork scans %ProgramFiles%\Claude\org-plugins only; %ProgramData% is
// invisible to it.
#[cfg(target_os = "windows")]
pub fn org_plugins_system() -> Option<PathBuf> {
    std::env::var_os("ProgramFiles")
        .map(|p| PathBuf::from(p).join("Claude").join("org-plugins"))
        .or_else(|| Some(PathBuf::from(r"C:\Program Files\Claude\org-plugins")))
}

#[cfg(target_os = "windows")]
pub fn org_plugins_user() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Claude").join("org-plugins"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Option-returning signature parity with the macos/windows cfg variants"
)]
pub fn org_plugins_system() -> Option<PathBuf> {
    Some(PathBuf::from("/opt/Claude/org-plugins"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn org_plugins_user() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .map(|base| base.join("Claude").join("org-plugins"))
}

#[must_use]
pub fn org_plugins_effective() -> Option<OrgPluginsLocation> {
    #[cfg(target_os = "macos")]
    {
        return org_plugins_system().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::System,
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(path) = org_plugins_system()
            && probe_writable(&path)
        {
            return Some(OrgPluginsLocation {
                path,
                scope: Scope::System,
            });
        }
        org_plugins_user().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::User,
        })
    }
}

#[cfg(target_os = "windows")]
#[must_use]
pub fn legacy_org_plugins_roots() -> Vec<PathBuf> {
    std::env::var_os("ProgramData")
        .map(|p| vec![PathBuf::from(p).join("Claude").join("org-plugins")])
        .unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
#[must_use]
pub const fn legacy_org_plugins_roots() -> Vec<PathBuf> {
    Vec::new()
}

#[must_use]
pub fn all_known_org_plugins_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(p) = org_plugins_system() {
        roots.push(p);
    }
    if let Some(p) = org_plugins_user() {
        roots.push(p);
    }
    roots.extend(legacy_org_plugins_roots());
    roots
}

pub const LEGACY_ORG_PLUGINS_METADATA: &[&str] = &[".systemprompt-bridge", ".systemprompt-cowork"];

// `Permissions::readonly` reports the file's own mode bits, not whether *this*
// process may create entries in the directory, so an unelevated Linux install
// used to select the root-owned system root and then fail. Probe by creating.
#[cfg(not(target_os = "macos"))]
fn probe_writable(path: &std::path::Path) -> bool {
    let mut candidate = Some(path);
    while let Some(dir) = candidate {
        match std::fs::metadata(dir) {
            Ok(metadata) if metadata.is_dir() => return can_create_in(dir),
            Ok(_) => return false,
            Err(_) => candidate = dir.parent(),
        }
    }
    false
}

#[cfg(not(target_os = "macos"))]
fn can_create_in(dir: &std::path::Path) -> bool {
    let probe = dir.join(format!(".sp-bridge-writeprobe-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            _ = std::fs::remove_file(&probe);
            true
        },
        Err(_) => false,
    }
}

// `None` means no Cowork install detected; callers must treat as a no-op, not
// an error.
#[must_use]
pub fn cowork3p_sessions_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|p| {
            PathBuf::from(p)
                .join("Claude-3p")
                .join("local-agent-mode-sessions")
        })
    }
    #[cfg(target_os = "macos")]
    {
        return dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Claude-3p")
                .join("local-agent-mode-sessions")
        });
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
            .map(|base| base.join("Claude-3p").join("local-agent-mode-sessions"))
    }
}

pub const COWORK_PLUGINS_SUBDIR: &str = "cowork_plugins";

pub const COWORK_ARTIFACTS_SUBDIR: &str = "cowork_artifacts";

// Always user-writable, unlike the admin-only org-plugins root on Windows.
#[cfg(target_os = "windows")]
#[must_use]
pub fn bridge_working_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join(crate::brand::brand().working_dir_name))
}

#[cfg(target_os = "macos")]
#[must_use]
pub fn bridge_working_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join("Library")
            .join("Application Support")
            .join(crate::brand::brand().working_dir_name)
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn bridge_working_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("state")))
        .map(|base| base.join(crate::brand::brand().working_dir_name))
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
    dirs::home_dir().map(|h| h.join(".claude"))
}

#[must_use]
pub fn claude_cli_plugins_dir() -> Option<PathBuf> {
    claude_cli_home().map(|h| h.join("plugins"))
}

#[must_use]
pub fn claude_cli_settings_path() -> Option<PathBuf> {
    claude_cli_home().map(|h| h.join("settings.json"))
}
