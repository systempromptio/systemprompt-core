//! Well-known bridge file locations and writable-directory probing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod dirs;

pub use dirs::{
    COWORK_ARTIFACTS_SUBDIR, COWORK_PLUGINS_SUBDIR, WORKSPACE_ARTIFACTS_SUBDIR,
    bridge_metadata_dir, bridge_staging_dir, bridge_working_dir, claude_cli_home,
    claude_cli_plugins_dir, claude_cli_settings_path, claude_code_policy_dir,
    cowork3p_sessions_root, workspace_artifacts_dir, workspace_dir,
};

use std::path::PathBuf;

pub const VERSION_SENTINEL: &str = "version.json";

fn org_plugins_system_override() -> Option<PathBuf> {
    std::env::var_os(crate::brand::brand().env("ORG_PLUGINS_SYSTEM")).map(PathBuf::from)
}
pub const LAST_SYNC_SENTINEL: &str = "last-sync.json";
pub const FIRST_RUN_SENTINEL: &str = "first-run.json";
pub const ONBOARDED_SENTINEL: &str = "onboarded.json";
pub const WINDOW_STATE_SENTINEL: &str = "window-state.json";
pub const TRAY_NOTICE_SENTINEL: &str = "tray-notice.json";
pub const USER_FRAGMENT: &str = "user.json";
pub const MCP_SERVERS_FRAGMENT: &str = "mcp-servers.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgPluginsLocation {
    pub path: PathBuf,
    pub scope: Scope,
    pub reason: FallbackReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

/// Why the resolved location was chosen.
///
/// `SystemUnwritable` records that the preferred system path exists in
/// principle but this process cannot write there — on Windows that path is
/// the only one Cowork scans, so callers syncing the Cowork host must treat
/// it as fatal rather than a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    Preferred,
    SystemUnwritable { system_path: PathBuf },
}

#[cfg(target_os = "macos")]
#[must_use]
pub fn org_plugins_system() -> Option<PathBuf> {
    org_plugins_system_override().or_else(|| {
        Some(PathBuf::from(
            "/Library/Application Support/Claude/org-plugins",
        ))
    })
}

#[cfg(target_os = "macos")]
pub fn org_plugins_user() -> Option<PathBuf> {
    crate::basedirs::data_home_override()
        .or_else(|| {
            crate::basedirs::home_dir().map(|h| h.join("Library").join("Application Support"))
        })
        .map(|base| base.join("Claude").join("org-plugins"))
}

// Why: Cowork scans %ProgramFiles%\Claude\org-plugins only; %ProgramData% is
// invisible to it.
#[cfg(target_os = "windows")]
pub fn org_plugins_system() -> Option<PathBuf> {
    org_plugins_system_override()
        .or_else(|| {
            std::env::var_os("ProgramFiles")
                .map(|p| PathBuf::from(p).join("Claude").join("org-plugins"))
        })
        .or_else(|| Some(PathBuf::from(r"C:\Program Files\Claude\org-plugins")))
}

#[cfg(target_os = "windows")]
pub fn org_plugins_user() -> Option<PathBuf> {
    crate::basedirs::data_home_override()
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .map(|base| base.join("Claude").join("org-plugins"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn org_plugins_system() -> Option<PathBuf> {
    org_plugins_system_override().or_else(|| Some(PathBuf::from("/opt/Claude/org-plugins")))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn org_plugins_user() -> Option<PathBuf> {
    crate::basedirs::data_home_override()
        .or_else(|| crate::basedirs::home_dir().map(|h| h.join(".local").join("share")))
        .map(|base| base.join("Claude").join("org-plugins"))
}

#[must_use]
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::needless_return,
        reason = "the macOS branch is the whole body; the return keeps the cfg arms symmetric"
    )
)]
pub fn org_plugins_effective() -> Option<OrgPluginsLocation> {
    #[cfg(target_os = "macos")]
    {
        return org_plugins_system().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::System,
            reason: FallbackReason::Preferred,
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let system = org_plugins_system();
        // Why: the system leaf may not exist yet — sync creates it on first
        // write — so a missing directory must not force user scope for a
        // process (e.g. elevated) that could create it.
        if let Some(path) = system.clone()
            && probe_writable(&path)
        {
            return Some(OrgPluginsLocation {
                path,
                scope: Scope::System,
                reason: FallbackReason::Preferred,
            });
        }
        org_plugins_user().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::User,
            reason: system.map_or(FallbackReason::Preferred, |system_path| {
                FallbackReason::SystemUnwritable { system_path }
            }),
        })
    }
}

#[must_use]
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::needless_return,
        reason = "the macOS branch is the whole body; the return keeps the cfg arms symmetric"
    )
)]
pub fn org_plugins_install_target() -> Option<OrgPluginsLocation> {
    #[cfg(target_os = "macos")]
    {
        return org_plugins_system().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::System,
            reason: FallbackReason::Preferred,
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let system = org_plugins_system();
        if let Some(path) = system.clone()
            && probe_writable(&path)
        {
            return Some(OrgPluginsLocation {
                path,
                scope: Scope::System,
                reason: FallbackReason::Preferred,
            });
        }
        org_plugins_user().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::User,
            reason: system.map_or(FallbackReason::Preferred, |system_path| {
                FallbackReason::SystemUnwritable { system_path }
            }),
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

// Why: `Permissions::readonly` reports the file's own mode bits, not whether
// this process may create entries in the directory. Probe by creating.
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
