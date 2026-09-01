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

// Why: the OpenCode managed tier has no vendor env override, so this bridge-
// owned one exists for tests and for operators staging a config into an
// image; it never changes what OpenCode itself reads.
pub(super) const MANAGED_DIR_OVERRIDE: &str = "SP_BRIDGE_OPENCODE_MANAGED_DIR";

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

// Why: the provider wire and its loopback base URL are what route inference
// through the bridge at all. The model list and default are negotiated from
// provider health, so their absence is a degraded profile, not a missing one.
pub(super) const REQUIRED_KEYS: &[&str] = &[PROVIDER_NPM, PROVIDER_BASE_URL];

pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

pub(super) fn managed_dir() -> PathBuf {
    if let Some(custom) = crate::basedirs::env_dir(MANAGED_DIR_OVERRIDE) {
        return custom;
    }
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

pub(super) fn managed_config_path() -> PathBuf {
    managed_dir().join(CONFIG_FILE)
}

pub(super) fn managed_jsonc_path() -> PathBuf {
    managed_dir().join(CONFIG_FILE_JSONC)
}

// Why: OpenCode's MDM domain sits above the managed file; a fleet that pushes
// one must probe as governed by it, not as missing. The override empties the
// list so a sandboxed probe never reads the live managed preferences.
#[cfg(target_os = "macos")]
pub(super) fn macos_managed_prefs_paths() -> Vec<PathBuf> {
    const DOMAIN: &str = "ai.opencode.managed.plist";
    const ROOT: &str = "/Library/Managed Preferences";
    if crate::basedirs::env_dir(MANAGED_DIR_OVERRIDE).is_some() {
        return Vec::new();
    }
    let mut paths = Vec::new();
    if let Some(user) = std::env::var_os("USER") {
        paths.push(PathBuf::from(ROOT).join(user).join(DOMAIN));
    }
    paths.push(PathBuf::from(ROOT).join(DOMAIN));
    paths
}

// Why: OpenCode uses `~/.config/opencode` on every OS — including macOS, where
// `dirs::config_dir` would point at `~/Library/Application Support` — with the
// XDG override honoured.
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

pub(super) fn make_uuids() -> (String, String) {
    let n = now_unix();
    let payload_uuid = format!(
        "ce0c{:08x}-0pc0-40pc-0pc0-{:012x}",
        n & 0xFFFF_FFFF,
        n ^ 0xC0DE_C0DE_C0DE_C0DEu64
    );
    let profile_uuid = format!(
        "ce0d{:08x}-0pc0-40pc-0pc0-{:012x}",
        (n ^ 0x9876_5432) & 0xFFFF_FFFF,
        n ^ 0xBEEF_FACE_BEEF_FACEu64
    );
    (payload_uuid, profile_uuid)
}
