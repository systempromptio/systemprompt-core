//! `OpenCode` path resolution and the bridge-owned key surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};


pub(super) const BINARY: &str = "opencode";
pub(super) const PROVIDER_ID: &str = "systemprompt";
pub(super) const NPM_PACKAGE: &str = "@ai-sdk/openai-compatible";
pub(super) const CONFIG_FILE: &str = "opencode.json";
pub(super) const CONFIG_FILE_JSONC: &str = "opencode.jsonc";
pub(super) const AUTH_FILE: &str = "auth.json";

pub(super) const PROVIDER_NPM: &str = "provider.systemprompt.npm";
pub(super) const PROVIDER_NAME: &str = "provider.systemprompt.name";
pub(super) const PROVIDER_BASE_URL: &str = "provider.systemprompt.options.baseURL";
pub(super) const PROVIDER_PROTOCOL_HEADER: &str =
    "provider.systemprompt.options.headers.x-inference-protocol";
pub(super) const PROVIDER_MODELS: &str = "provider.systemprompt.models";
pub(super) const DEFAULT_MODEL: &str = "model";
pub(super) const ENABLED_PROVIDERS: &str = "enabled_providers";

pub(super) const PROVIDER_API_KEY: &str = "provider.systemprompt.options.apiKey";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    PROVIDER_NPM,
    PROVIDER_NAME,
    PROVIDER_BASE_URL,
    PROVIDER_PROTOCOL_HEADER,
    PROVIDER_MODELS,
    PROVIDER_API_KEY,
    DEFAULT_MODEL,
    ENABLED_PROVIDERS,
];

#[must_use]
pub(super) fn role_of(key: &str) -> crate::drift::EntryRole {
    use crate::drift::EntryRole;
    match key {
        PROVIDER_BASE_URL => EntryRole::Endpoint,
        PROVIDER_MODELS | DEFAULT_MODEL => EntryRole::Models,
        PROVIDER_API_KEY => EntryRole::Secret,
        _ => EntryRole::Setting,
    }
}

// Why: OpenCode 2.x does not load the admin-managed tier, so a provider block
// in it is dead config that only uninstall and
// doctor still look at. `[opencode] managed_dir` relocates it (a test sandbox);
// the platform default applies only when the config does not set it.
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

pub(super) fn skills_dir() -> PathBuf {
    user_dir().join("skills")
}

pub(super) fn data_dir() -> PathBuf {
    let base = crate::basedirs::data_home_override()
        .or_else(|| crate::basedirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join(BINARY)
}

// Why: OpenCode 2.x imports `auth.json` into its own store once and never
// reads it again, so a key there cannot rotate; it is only ever removed.
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

// Why: the Electron desktop app is a per-user NSIS install whose folder is the
// npm scope of its package (`@opencodedesktop`), not the product name, and it
// puts no `opencode` binary on PATH.
pub(super) fn desktop_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(
            PathBuf::from(local)
                .join("Programs")
                .join("@opencodedesktop")
                .join("OpenCode.exe"),
        );
    }
    out
}

pub(super) const fn desktop_locator(
    candidates: &[PathBuf],
) -> crate::integration::app_launch::AppLocator<'_> {
    crate::integration::app_launch::AppLocator {
        macos_name: "OpenCode",
        windows_name: "OpenCode",
        windows_candidates: candidates,
        linux_bin: BINARY,
        msix_family: None,
        msix_app_id: "",
    }
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
