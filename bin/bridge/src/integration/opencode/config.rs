//! `OpenCode` path resolution and the bridge-owned key surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::integration::host_app::HostConfigSchema;

pub(super) const BINARY: &str = "opencode";
pub(super) const PROVIDER_ID: &str = "systemprompt";
pub(super) const NPM_PACKAGE: &str = "@ai-sdk/openai-compatible";
pub(super) const CONFIG_FILE: &str = "opencode.json";
pub(super) const CONFIG_FILE_JSONC: &str = "opencode.jsonc";
pub(super) const AUTH_FILE: &str = "auth.json";

pub(super) const PROVIDER_NPM: &str = "provider.systemprompt.npm";
pub(super) const PROVIDER_BASE_URL: &str = "provider.systemprompt.options.baseURL";
pub(super) const PROVIDER_PROTOCOL_HEADER: &str =
    "provider.systemprompt.options.headers.x-inference-protocol";
pub(super) const PROVIDER_MODELS: &str = "provider.systemprompt.models";
pub(super) const DEFAULT_MODEL: &str = "model";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    PROVIDER_NPM,
    PROVIDER_BASE_URL,
    PROVIDER_PROTOCOL_HEADER,
    PROVIDER_MODELS,
    DEFAULT_MODEL,
];

pub(super) const REQUIRED_KEYS: &[&str] = &[PROVIDER_NPM, PROVIDER_BASE_URL];

pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

// Why: `[opencode] managed_dir` in the bridge config names a nonstandard
// managed tier (a relocated /etc, a test sandbox); the platform default
// applies only when the config does not set it.
pub(super) fn managed_dir() -> Result<PathBuf, crate::config::ConfigReadError> {
    let cfg = crate::config::load()?;
    if let Some(custom) = cfg.opencode.and_then(|o| o.managed_dir) {
        return Ok(custom);
    }
    Ok(platform_managed_dir())
}

fn platform_managed_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/opencode")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("ProgramData")
            .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from)
            .join("opencode")
    } else {
        PathBuf::from("/etc/opencode")
    }
}

pub(super) fn managed_config_path() -> Result<PathBuf, crate::config::ConfigReadError> {
    Ok(managed_dir()?.join(CONFIG_FILE))
}

// Why: OpenCode's macOS MDM preferences take precedence over the managed file.
#[cfg(target_os = "macos")]
pub(super) fn macos_managed_prefs_paths() -> Vec<PathBuf> {
    const DOMAIN: &str = "ai.opencode.managed.plist";
    const ROOT: &str = "/Library/Managed Preferences";
    if crate::config::load()
        .ok()
        .and_then(|c| c.opencode)
        .is_some_and(|o| o.managed_dir.is_some())
    {
        return Vec::new();
    }
    let mut paths = Vec::new();
    if let Some(user) = std::env::var_os("USER") {
        paths.push(PathBuf::from(ROOT).join(user).join(DOMAIN));
    }
    paths.push(PathBuf::from(ROOT).join(DOMAIN));
    paths
}

// Why: OpenCode uses ~/.config/opencode even on macOS, honoring
// XDG_CONFIG_HOME.
pub(super) fn user_dir() -> PathBuf {
    let base = crate::basedirs::config_home_override()
        .or_else(|| crate::basedirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join(BINARY)
}

pub(super) fn user_config_path() -> PathBuf {
    user_dir().join(CONFIG_FILE)
}

// Why: OpenCode deep-merges the user's global config beneath the managed
// tier, so a provider block written here adds the live models to an admin
// file this process cannot rewrite; the admin file's own entries and default
// `model` still win until it is refreshed with elevation.
pub(super) fn user_tier_path(managed: &std::path::Path) -> Option<PathBuf> {
    let path = user_config_path();
    (path != managed).then_some(path)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn fallback_config_path(managed: &std::path::Path) -> Option<PathBuf> {
    let path = user_config_path();
    if path == managed {
        return None;
    }
    Some(path)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const fn fallback_config_path(_managed: &std::path::Path) -> Option<PathBuf> {
    None
}

pub(super) fn skills_dir() -> PathBuf {
    user_dir().join("skills")
}

pub(super) fn data_dir() -> PathBuf {
    let base = crate::basedirs::data_home_override()
        .or_else(|| crate::basedirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join(BINARY)
}

pub(super) fn auth_json_path() -> PathBuf {
    data_dir().join(AUTH_FILE)
}

pub(super) fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(home) = crate::basedirs::home_dir() {
        dirs.push(home.join(".opencode").join("bin"));
        dirs.push(home.join(".bun").join("bin"));
        dirs.push(home.join(".npm-global").join("bin"));
        dirs.push(home.join("scoop").join("shims"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
