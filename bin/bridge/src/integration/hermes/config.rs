//! Hermes Agent Desktop `config.yaml` fragment generation and path resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::integration::host_app::HostConfigSchema;

// Why: Hermes resolves model.provider before its base_url; custom endpoints
// require a named provider.
pub const PROVIDER_ENTRY: &str = "systemprompt-gateway";
pub const MODEL_PROVIDER: &str = "model.provider";
// Why: Hermes' model.default overrides model.model, even when both parse.
pub const MODEL_NAME: &str = "model.default";

pub const PROVIDER_BASE_URL: &str = "providers.systemprompt-gateway.base_url";
pub const PROVIDER_API_MODE: &str = "providers.systemprompt-gateway.api_mode";
pub const PROVIDER_KEY_ENV: &str = "providers.systemprompt-gateway.key_env";

// Why: Hermes accepts chat_completions as api_mode; openai is silently
// discarded.
pub const API_MODE_VALUE: &str = "chat_completions";

// Why: Hermes restricts the OPENAI_API_KEY fallback to OpenAI/Azure hosts;
// loopback requires key_env.
pub(super) const KEY_ENV_VALUE: &str = ENV_API_KEY;

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    MODEL_PROVIDER,
    MODEL_NAME,
    PROVIDER_BASE_URL,
    PROVIDER_API_MODE,
    PROVIDER_KEY_ENV,
];

pub(super) const REQUIRED_KEYS: &[&str] = &[
    MODEL_PROVIDER,
    PROVIDER_BASE_URL,
    PROVIDER_API_MODE,
    PROVIDER_KEY_ENV,
];

pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

// Why: Hermes defaults to %LOCALAPPDATA%\hermes on Windows and ~/.hermes
// elsewhere.
pub(super) fn hermes_home() -> PathBuf {
    if let Some(custom) = std::env::var_os("HERMES_HOME") {
        return PathBuf::from(custom);
    }
    if cfg!(target_os = "windows")
        && let Some(local) = crate::basedirs::data_local_dir()
    {
        return local.join("hermes");
    }
    if let Some(home) = crate::basedirs::home_dir() {
        return home.join(".hermes");
    }
    PathBuf::from(".hermes")
}

pub(super) fn config_yaml_path() -> PathBuf {
    config_yaml_path_in(&hermes_home())
}

pub(super) fn config_yaml_path_in(home: &std::path::Path) -> PathBuf {
    home.join("config.yaml")
}

pub(super) fn env_path_in(home: &std::path::Path) -> PathBuf {
    home.join(".env")
}

pub(super) fn skills_dir() -> PathBuf {
    hermes_home().join("skills")
}

pub const ENV_API_KEY: &str = "OPENAI_API_KEY";

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(super) fn make_uuids() -> (String, String) {
    let n = now_unix();
    let payload_uuid = format!(
        "ce0c{:08x}-h3rm-4h3r-h3r0-{:012x}",
        n & 0xFFFF_FFFF,
        n ^ 0xC0DE_C0DE_C0DE_C0DEu64
    );
    let profile_uuid = format!(
        "ce0d{:08x}-h3rm-4h3r-h3r0-{:012x}",
        (n ^ 0x9876_5432) & 0xFFFF_FFFF,
        n ^ 0xBEEF_FACE_BEEF_FACEu64
    );
    (payload_uuid, profile_uuid)
}
