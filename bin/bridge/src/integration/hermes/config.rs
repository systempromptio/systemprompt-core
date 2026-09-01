//! Hermes Agent Desktop `config.yaml` fragment generation and path resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::integration::host_app::HostConfigSchema;

// Why: Hermes reaches a non-built-in endpoint through a *named* entry under
// `providers:`, selected by `model.provider`. Writing `model.base_url` with
// `model.provider` left at its default `auto` does not route anything: Hermes
// resolves the provider first and only then consults that provider's base_url.
pub(super) const PROVIDER_ENTRY: &str = "systemprompt-gateway";
pub(super) const MODEL_PROVIDER: &str = "model.provider";
// Why: `model.default` — not `model.model`. Both names parse, but when the two
// are present `default` wins, and Hermes' own installed config.yaml always
// ships a `default`. Writing `model` alone is therefore silently inert.
pub(super) const MODEL_NAME: &str = "model.default";

pub(super) const PROVIDER_BASE_URL: &str = "providers.systemprompt-gateway.base_url";
pub(super) const PROVIDER_API_MODE: &str = "providers.systemprompt-gateway.api_mode";
pub(super) const PROVIDER_KEY_ENV: &str = "providers.systemprompt-gateway.key_env";

// Why: Hermes' api_mode vocabulary is `chat_completions` / `codex_responses` /
// `anthropic_messages` (plus `bedrock_converse`). "openai" is not a member and
// is silently discarded, so the wire format has to be named explicitly.
pub(super) const API_MODE_VALUE: &str = "chat_completions";

// Why: the loopback secret stays in `HERMES_HOME/.env` (0600) rather than in
// config.yaml, and `key_env` is how a named provider reads it from there.
// Hermes host-gates the bare `OPENAI_API_KEY` fallback to openai.com/azure, so
// for a 127.0.0.1 endpoint an explicit `key_env` is the only path that resolves
// the credential — without it Hermes sends its `no-key-required` placeholder
// and the bridge proxy answers 403.
pub(super) const KEY_ENV_VALUE: &str = ENV_API_KEY;

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    MODEL_PROVIDER,
    MODEL_NAME,
    PROVIDER_BASE_URL,
    PROVIDER_API_MODE,
    PROVIDER_KEY_ENV,
];

// Why: the provider selection, its endpoint and its wire format are what make
// Hermes route inference through the bridge at all; without any of them the
// profile is not doing its job. `key_env` is required too — the request reaches
// the proxy without it and is rejected. The concrete model name stays optional
// (it may be selected in-app).
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
