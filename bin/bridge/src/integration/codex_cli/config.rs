use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::integration::host_app::HostConfigSchema;

pub(super) const PROVIDER_BASE_URL: &str = "model_providers.systemprompt.base_url";
pub(super) const PROVIDER_WIRE_API: &str = "model_providers.systemprompt.wire_api";
pub(super) const PROVIDER_AUTH_COMMAND: &str = "model_providers.systemprompt.auth.command";
pub(super) const PROVIDER_AUTH_REFRESH: &str =
    "model_providers.systemprompt.auth.refresh_interval_ms";
pub(super) const PROVIDER_HEADER_TENANT: &str =
    "model_providers.systemprompt.http_headers.x-tenant";
pub(super) const OTEL_EXPORTER: &str = "otel.exporter";
pub(super) const OTEL_LOG_USER_PROMPT: &str = "otel.log_user_prompt";
pub(super) const OTEL_ENDPOINT: &str = "otel.exporter.systemprompt.endpoint";
pub(super) const ANALYTICS_ENABLED: &str = "analytics.enabled";
pub(super) const TOP_MODEL_PROVIDER: &str = "model_provider";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    PROVIDER_BASE_URL,
    PROVIDER_WIRE_API,
    PROVIDER_AUTH_COMMAND,
    PROVIDER_AUTH_REFRESH,
    PROVIDER_HEADER_TENANT,
    OTEL_EXPORTER,
    OTEL_LOG_USER_PROMPT,
    OTEL_ENDPOINT,
    ANALYTICS_ENABLED,
    TOP_MODEL_PROVIDER,
];

pub(super) const REQUIRED_KEYS: &[&str] = &[
    PROVIDER_BASE_URL,
    PROVIDER_WIRE_API,
    PROVIDER_AUTH_COMMAND,
    TOP_MODEL_PROVIDER,
];

pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

pub(super) fn codex_home() -> PathBuf {
    if let Some(custom) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(custom);
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".codex");
    }
    PathBuf::from(".codex")
}

pub(super) fn user_config_path() -> PathBuf {
    codex_home().join("config.toml")
}

pub(super) fn managed_config_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        codex_home().join("managed_config.toml")
    } else {
        PathBuf::from("/etc/codex/managed_config.toml")
    }
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(super) fn make_uuids() -> (String, String) {
    let n = now_unix();
    let payload_uuid = format!(
        "ce0c{:08x}-cdx0-4cdx-cdx0-{:012x}",
        n & 0xFFFF_FFFF,
        n ^ 0xC0DE_C0DE_C0DE_C0DEu64
    );
    let profile_uuid = format!(
        "ce0d{:08x}-cdx0-4cdx-cdx0-{:012x}",
        (n ^ 0x9876_5432) & 0xFFFF_FFFF,
        n ^ 0xBEEF_FACE_BEEF_FACEu64
    );
    (payload_uuid, profile_uuid)
}

pub(super) fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == PROVIDER_HEADER_TENANT {
        let len = raw.chars().filter(|c| !c.is_whitespace()).count();
        return format!("<present, {len} chars>");
    }
    raw
}
