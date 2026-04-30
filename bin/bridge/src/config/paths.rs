use std::path::PathBuf;

pub const METADATA_DIR: &str = ".systemprompt-bridge";
pub const VERSION_SENTINEL: &str = "version.json";
pub const LAST_SYNC_SENTINEL: &str = "last-sync.json";
pub const USER_FRAGMENT: &str = "user.json";
pub const STAGING_DIR: &str = ".staging";

pub const SYNTHETIC_PLUGIN_NAME: &str = "systemprompt-managed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgPluginsLocation {
    pub path: PathBuf,
    pub scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "windows")]
pub fn org_plugins_system() -> Option<PathBuf> {
    std::env::var_os("ProgramData")
        .map(|p| PathBuf::from(p).join("Claude").join("org-plugins"))
        .or_else(|| Some(PathBuf::from(r"C:\ProgramData\Claude\org-plugins")))
}

#[cfg(target_os = "windows")]
pub fn org_plugins_user() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Claude").join("org-plugins"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
#[allow(clippy::unnecessary_wraps)]
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
        if let Some(path) = org_plugins_system() {
            if probe_writable(&path) {
                return Some(OrgPluginsLocation {
                    path,
                    scope: Scope::System,
                });
            }
        }
        org_plugins_user().map(|path| OrgPluginsLocation {
            path,
            scope: Scope::User,
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn probe_writable(path: &std::path::Path) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        return metadata.is_dir() && !metadata.permissions().readonly();
    }
    if let Some(parent) = path.parent() {
        if let Ok(metadata) = std::fs::metadata(parent) {
            return metadata.is_dir() && !metadata.permissions().readonly();
        }
    }
    false
}

#[must_use]
pub fn metadata_dir(org_plugins: &std::path::Path) -> PathBuf {
    org_plugins.join(METADATA_DIR)
}

#[must_use]
pub fn staging_dir(org_plugins: &std::path::Path) -> PathBuf {
    org_plugins.join(STAGING_DIR)
}
