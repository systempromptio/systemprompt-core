//! Hermes Agent Desktop `config.yaml` fragment generation and path resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::integration::host_app::HostConfigSchema;

// Why: the bridge-owned `model` keys are addressed with the same dotted
// convention the Codex host uses for its TOML surface.
pub(super) const MODEL_BASE_URL: &str = "model.base_url";
pub(super) const MODEL_API_MODE: &str = "model.api_mode";
pub(super) const MODEL_NAME: &str = "model.model";

// Why: `openai` selects the chat/completions wire format the gateway serves at
// `/v1/chat/completions`, matching the host's single accepted surface.
pub(super) const API_MODE_VALUE: &str = "openai";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[MODEL_BASE_URL, MODEL_API_MODE, MODEL_NAME];

// Why: base_url and api_mode are what make Hermes route inference through the
// bridge at all; without either the profile is not doing its job. The concrete
// model name is optional (it may be selected in-app), so it is not required.
pub(super) const REQUIRED_KEYS: &[&str] = &[MODEL_BASE_URL, MODEL_API_MODE];

pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

// Why: Hermes resolves HERMES_HOME to %LOCALAPPDATA%\hermes on Windows and
// ~/.hermes elsewhere, with an explicit HERMES_HOME override taking precedence.
// `data_local_dir` honours the same override discipline as the rest of the
// bridge and maps to %LOCALAPPDATA% on Windows.
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
    hermes_home().join("config.yaml")
}

pub(super) fn env_path() -> PathBuf {
    hermes_home().join(".env")
}

pub(super) fn skills_dir() -> PathBuf {
    hermes_home().join("skills")
}

pub(super) const ENV_API_KEY: &str = "OPENAI_API_KEY";

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
