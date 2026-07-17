//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::host_app::HostConfigSchema;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const API_KEY_KEY: &str = "inferenceGatewayApiKey";

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    API_KEY_KEY,
    "inferenceGatewayAuthScheme",
    "inferenceCustomHeaders",
    "inferenceModels",
];

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

const DEFAULT_MODELS: &[&str] = &["claude-opus-4-7", "claude-sonnet-4-6", "claude-haiku-4-5"];

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
    pub api_key_fp: Option<String>,
}

/// `None` when there is no baked key or the proxy has not started — the caller
/// must treat that as "cannot assert staleness", never as stale.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn secret_freshness(installed_api_key_fp: Option<&str>) -> Option<bool> {
    let installed = installed_api_key_fp?;
    let live = crate::proxy::secret::for_profile().ok()?;
    Some(installed == crate::proxy::secret::fingerprint(live.as_str()))
}

pub use crate::integration::host_app::ProfileGenInputs;

#[must_use]
pub fn default_models() -> Vec<String> {
    DEFAULT_MODELS.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// Pid + monotonic counter keep concurrent stagers in the shared temp dir from
// racing on the same `File::create` path.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn unique_stem() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        now_unix(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn make_uuids() -> (String, String) {
    let n = now_unix();
    let payload_uuid = format!(
        "ce0a{:08x}-cwk0-4cwk-cwk0-{:012x}",
        n & 0xFFFF_FFFF,
        n ^ 0xDEAD_BEEF_CAFE_BABEu64
    );
    let profile_uuid = format!(
        "ce0b{:08x}-cwk0-4cwk-cwk0-{:012x}",
        (n ^ 0x1234_5678) & 0xFFFF_FFFF,
        n ^ 0xFEED_FACE_DEAD_C0DEu64
    );
    (payload_uuid, profile_uuid)
}
