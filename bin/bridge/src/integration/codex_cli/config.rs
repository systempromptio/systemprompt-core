//! Codex CLI `config.toml` fragment generation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
pub(super) const OTEL_LOG_USER_PROMPT: &str = "otel.log_user_prompt";
pub(super) const OTEL_ENDPOINT: &str = "otel.exporter.otlp-http.endpoint";
pub(super) const OTEL_PROTOCOL: &str = "otel.exporter.otlp-http.protocol";
pub(super) const ANALYTICS_ENABLED: &str = "analytics.enabled";
pub(super) const TOP_MODEL_PROVIDER: &str = "model_provider";
pub(super) const APPROVAL_POLICY: &str = "approval_policy";
pub(super) const SANDBOX_MODE: &str = "sandbox_mode";
pub(super) const SANDBOX_NETWORK_ACCESS: &str = "sandbox_workspace_write.network_access";

// Why: approval prompts are a fleet policy, not a per-developer preference. An
// unattended Codex that stops to ask cannot be governed centrally, and every
// prompt answered locally is a decision the gateway never sees.
pub(super) const APPROVAL_POLICY_VALUE: &str = "never";
// Why: deliberately not `danger-full-access` — suppressing the prompts does not
// mean surrendering the sandbox. Commands stay confined to the workspace, so an
// unattended run's blast radius is bounded by the checkout it runs in.
pub(super) const SANDBOX_MODE_VALUE: &str = "workspace-write";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    PROVIDER_BASE_URL,
    PROVIDER_WIRE_API,
    PROVIDER_AUTH_COMMAND,
    PROVIDER_AUTH_REFRESH,
    PROVIDER_HEADER_TENANT,
    OTEL_LOG_USER_PROMPT,
    OTEL_ENDPOINT,
    OTEL_PROTOCOL,
    ANALYTICS_ENABLED,
    TOP_MODEL_PROVIDER,
    APPROVAL_POLICY,
    SANDBOX_MODE,
    SANDBOX_NETWORK_ACCESS,
];

pub(super) const REQUIRED_KEYS: &[&str] = &[
    PROVIDER_BASE_URL,
    PROVIDER_WIRE_API,
    PROVIDER_AUTH_COMMAND,
    TOP_MODEL_PROVIDER,
    APPROVAL_POLICY,
    SANDBOX_MODE,
];

pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

pub(super) fn codex_home() -> PathBuf {
    if let Some(custom) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(custom);
    }
    if let Some(home) = crate::basedirs::home_dir() {
        return home.join(".codex");
    }
    PathBuf::from(".codex")
}

pub(super) fn user_config_path() -> PathBuf {
    codex_home().join("config.toml")
}

// Why: Codex defines no admin-scope path on Windows, so managed config lands
// user-scope under CODEX_HOME there.
pub(super) fn managed_config_path() -> PathBuf {
    if let Some(custom) = std::env::var_os("CODEX_SYSTEM_CONFIG") {
        return PathBuf::from(custom);
    }
    if cfg!(target_os = "windows") {
        codex_home().join("managed_config.toml")
    } else {
        PathBuf::from("/etc/codex/config.toml")
    }
}

// Why: on macOS the installed profile is not a TOML file on disk — the payload
// carries the config as base64 under `config_toml_base64` inside the
// `com.openai.codex` managed-preference plist. So `managed_config_path`'s
// `/etc/codex/config.toml` never exists there however successful the install
// was, which is what made every re-verify report "profile not installed"
// seconds after reporting the profile loaded. User scope precedes device scope.
#[cfg(target_os = "macos")]
pub(super) fn macos_managed_prefs_paths() -> Vec<PathBuf> {
    const DOMAIN: &str = "com.openai.codex.plist";
    const ROOT: &str = "/Library/Managed Preferences";
    let mut paths = Vec::new();
    if let Some(user) = std::env::var_os("USER") {
        paths.push(PathBuf::from(ROOT).join(user).join(DOMAIN));
    }
    paths.push(PathBuf::from(ROOT).join(DOMAIN));
    paths
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
