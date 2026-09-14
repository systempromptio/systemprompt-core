//! Shared Claude Desktop policy values and request sequencing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::host_app::HostConfigSchema;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const HOST_ID: &str = "claude-desktop";

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const API_KEY_KEY: &str = crate::cowork_compat::POLICY_API_KEY;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const KEYS_OF_INTEREST: &[&str] = crate::cowork_compat::POLICY_KEYS;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const REQUIRED_KEYS: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    API_KEY_KEY,
    "inferenceModels",
];

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: REQUIRED_KEYS,
    display_keys: KEYS_OF_INTEREST,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
    pub api_key_fp: Option<String>,
    pub probe_error: Option<String>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn secret_freshness(
    installed_api_key_fp: Option<&str>,
    env: &crate::integration::host_app::ProbeEnv,
) -> crate::integration::host_app::Freshness {
    let live = env.host_token_fingerprint(&crate::ids::HostId::new(HOST_ID));
    crate::integration::host_app::Freshness::compare(
        installed_api_key_fp,
        live.as_deref(),
        "loopback secret",
    )
}

pub use crate::integration::host_app::ProfileGenInputs;

#[must_use]
pub fn default_models() -> Vec<String> {
    crate::install::default_inference_models()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == API_KEY_KEY {
        return format!(
            "<present, {} chars>",
            raw.chars().filter(|c| !c.is_whitespace()).count()
        );
    }
    raw
}
