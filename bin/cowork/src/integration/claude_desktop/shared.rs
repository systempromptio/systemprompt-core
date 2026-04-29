#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::host_app::HostConfigSchema;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const CODE_DOMAIN: &str = "com.anthropic.claudecode";

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceGatewayHeaders",
    "inferenceModels",
    "deploymentOrganizationUuid",
];

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const REQUIRED_KEYS: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    "inferenceGatewayApiKey",
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
}

pub use crate::integration::host_app::ProfileGenInputs;

#[must_use]
pub fn default_models() -> Vec<String> {
    DEFAULT_MODELS.iter().map(|s| (*s).to_string()).collect()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == "inferenceGatewayApiKey" {
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
