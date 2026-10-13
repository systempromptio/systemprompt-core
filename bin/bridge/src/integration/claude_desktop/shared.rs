//! Shared Claude Desktop policy values and request sequencing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::time::{SystemTime, UNIX_EPOCH};


#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const API_KEY_KEY: &str = crate::cowork_compat::POLICY_API_KEY;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) const KEYS_OF_INTEREST: &[&str] = crate::cowork_compat::POLICY_KEYS;

// Why: the probe reads every key the policy can carry, so a key a release
// adds to the policy is compared from the release that writes it.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn keys_to_read(desired: Option<&crate::drift::DesiredProfile>) -> Vec<&str> {
    let mut keys: Vec<&str> = KEYS_OF_INTEREST
        .iter()
        .chain(crate::install::mdm::policy::WRITTEN_POLICY_KEYS)
        .copied()
        .collect();
    if let Some(desired) = desired {
        keys.extend(desired.keys());
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
    pub observed: BTreeMap<String, String>,
    pub api_key_fp: Option<String>,
    pub probe_error: Option<String>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl DomainRead {
    pub(super) fn record(&mut self, name: &str, text: PlistValueTexts) {
        let observed = if name == API_KEY_KEY {
            let fp = crate::proxy::secret::fingerprint(text.compare.trim());
            self.api_key_fp = Some(fp.clone());
            fp
        } else {
            text.compare
        };
        self.observed.insert(name.to_owned(), observed);
        self.keys
            .insert(name.to_owned(), redact_if_sensitive(name, text.display));
    }
}

/// A managed-policy value read back from the host: the text a person reads
/// and the text [`crate::drift::diff`] compares against the desired profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlistValueTexts {
    pub display: String,
    pub compare: String,
}

impl PlistValueTexts {
    #[must_use]
    pub fn same(text: String) -> Self {
        Self {
            display: text.clone(),
            compare: text,
        }
    }
}

// Why: Claude Desktop's desired profile renders a list as JSON text, so a
// plist array compared as its comma-joined display text never matched and
// every string-list key (`inferenceModels`, `coworkEgressAllowedHosts`, …)
// read as drift that no re-apply could clear. Only the display is joined.
// JSON: Claude Desktop plist — native preferences read back as JSON.
#[must_use]
pub fn plist_value_texts(value: &serde_json::Value) -> PlistValueTexts {
    match value {
        serde_json::Value::String(s) => PlistValueTexts::same(s.clone()),
        serde_json::Value::Array(items) if items.iter().all(serde_json::Value::is_string) => {
            PlistValueTexts {
                display: items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
                compare: value.to_string(),
            }
        },
        other => PlistValueTexts::same(other.to_string()),
    }
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
