use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub(super) const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";
pub(super) const CODE_DOMAIN: &str = "com.anthropic.claudecode";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceGatewayHeaders",
    "inferenceModels",
    "deploymentOrganizationUuid",
];

const DEFAULT_MODELS: &[&str] = &["claude-opus-4-7", "claude-sonnet-4-6", "claude-haiku-4-5"];

#[derive(Debug, Clone, Serialize, Default)]
pub struct ManagedPrefsState {
    pub desktop: ManagedDomain,
    pub code: ManagedDomain,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ManagedDomain {
    /// Reverse-DNS string read directly from the OS (`com.anthropic.claudecode` etc.).
    /// Wire-format-critical: must round-trip exactly to disk and back.
    pub domain: String,
    pub source_path: Option<String>,
    pub installed: bool,
    /// Free-form prefs key/value map mirrored from the OS — case-preserved for wire fidelity
    /// with macOS plist / Windows registry payloads.
    pub keys: BTreeMap<String, String>,
    pub missing_required: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    /// Loopback URL constructed locally as `http://localhost:{port}`; not part of any wire payload
    /// from the gateway, used only to render macOS/Windows profile templates.
    pub gateway_base_url: String,
    /// Loopback bearer secret minted by the proxy; rendered into a generated MDM profile and
    /// kept as a String at this boundary because it is consumed verbatim by template rendering.
    pub api_key: String,
    /// Model identifiers returned by the gateway profile endpoint; written verbatim into the
    /// generated MDM profile.
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
}

impl ProfileGenInputs {
    pub fn builder(
        gateway_base_url: impl Into<String>,
        api_key: impl Into<String>,
        models: Vec<String>,
    ) -> ProfileGenInputsBuilder {
        ProfileGenInputsBuilder {
            gateway_base_url: gateway_base_url.into(),
            api_key: api_key.into(),
            models,
            organization_uuid: None,
        }
    }
}

pub struct ProfileGenInputsBuilder {
    gateway_base_url: String,
    api_key: String,
    models: Vec<String>,
    organization_uuid: Option<String>,
}

impl ProfileGenInputsBuilder {
    pub fn with_organization_uuid(mut self, uuid: impl Into<String>) -> Self {
        self.organization_uuid = Some(uuid.into());
        self
    }

    pub fn build(self) -> ProfileGenInputs {
        ProfileGenInputs {
            gateway_base_url: self.gateway_base_url,
            api_key: self.api_key,
            models: self.models,
            organization_uuid: self.organization_uuid,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedProfile {
    pub path: String,
    pub bytes: usize,
    pub payload_uuid: String,
    pub profile_uuid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateProfileBody {
    #[serde(default)]
    pub gateway_base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}

pub fn default_models() -> Vec<String> {
    DEFAULT_MODELS.iter().map(|s| (*s).to_string()).collect()
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(super) fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == "inferenceGatewayApiKey" {
        return format!(
            "<present, {} chars>",
            raw.chars().filter(|c| !c.is_whitespace()).count()
        );
    }
    raw
}

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
