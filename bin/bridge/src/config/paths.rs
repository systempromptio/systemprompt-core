//! Well-known bridge file locations and writable-directory probing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

pub const VERSION_SENTINEL: &str = "version.json";

// Operator/test override for the system org-plugins root. Without it the
// system root is a fixed machine-wide path, so any test or nonstandard install
// on a host with a writable parent would collide with the real location.
fn org_plugins_system_override() -> Option<PathBuf> {
    std::env::var_os(crate::brand::brand().env("ORG_PLUGINS_SYSTEM")).map(PathBuf::from)
}
pub const LAST_SYNC_SENTINEL: &str = "last-sync.json";
/// Marks that first-use provisioning has run on this machine.
pub const FIRST_RUN_SENTINEL: &str = "first-run.json";
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

// Cowork scans %ProgramFiles%\Claude\org-plugins only; %ProgramData% is
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

// The system scope wins only when its directory already exists: consumers of
// the effective location (sync, doctor, GUI, status) must never adopt a system
// path that no install provisioned — on hosts where the system parent happens
// to be writable (CI runners, permissive /opt) that would silently shadow the
// user scope. `org_plugins_install_target` keeps the create-if-possible probe.
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
            && path.is_dir()
            && can_create_in(&path)
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

// Install may create the system directory, so it probes the nearest existing
// ancestor for writability instead of requiring the directory to exist.
#[must_use]
pub fn org_plugins_install_target() -> Option<OrgPluginsLocation> {
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

// `None` means Cowork cannot be present — either no install detected, or the
// platform ships no Cowork build at all. Callers must treat it as a no-op, not
// an error.
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
    // Cowork ships macOS and Windows builds only. Deriving an XDG-style Linux
    // location would name a directory no install can ever create, which reads
    // downstream as "Cowork present but broken" rather than "not applicable".
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

pub const COWORK_PLUGINS_SUBDIR: &str = "cowork_plugins";

pub const COWORK_ARTIFACTS_SUBDIR: &str = "cowork_artifacts";

// Always user-writable, unlike the admin-only org-plugins root on Windows.
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
