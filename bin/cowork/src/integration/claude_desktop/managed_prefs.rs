use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

const MANAGED_PREFS_ROOT: &str = "/Library/Managed Preferences";

const KEYS_OF_INTEREST: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceGatewayHeaders",
    "inferenceModels",
    "deploymentOrganizationUuid",
];

#[derive(Debug, Clone, Serialize, Default)]
pub struct ManagedPrefsState {
    pub desktop: ManagedDomain,
    pub code: ManagedDomain,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ManagedDomain {
    pub domain: String,
    pub plist_path: Option<String>,
    pub installed: bool,
    pub keys: BTreeMap<String, String>,
    pub missing_required: Vec<String>,
}

fn managed_prefs_candidates(domain: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            out.push(
                PathBuf::from(MANAGED_PREFS_ROOT)
                    .join(&user)
                    .join(format!("{domain}.plist")),
            );
        }
    }
    out.push(PathBuf::from(MANAGED_PREFS_ROOT).join(format!("{domain}.plist")));
    out
}

pub(super) fn read_domain(domain: &str, required: &[&str]) -> ManagedDomain {
    let mut out = ManagedDomain {
        domain: domain.to_string(),
        ..Default::default()
    };

    let plist_path = managed_prefs_candidates(domain)
        .into_iter()
        .find(|p| p.exists());

    if let Some(path) = plist_path.as_ref() {
        out.plist_path = Some(path.display().to_string());
        out.installed = true;
    }

    let plist_json = plist_path
        .as_deref()
        .and_then(read_plist_as_json)
        .unwrap_or(serde_json::Value::Null);

    for key in KEYS_OF_INTEREST {
        if let Some(val) = read_key_value(&plist_json, domain, key) {
            out.keys.insert(key.to_string(), val);
        }
    }

    out.missing_required = required
        .iter()
        .filter(|k| !out.keys.contains_key(**k))
        .map(|k| (*k).to_string())
        .collect();

    out
}

// JSON: protocol boundary — plist content is operator-defined; shape varies per
// managed-prefs domain so values flow as `serde_json::Value`.
fn read_plist_as_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let output = Command::new("/usr/bin/plutil")
        .arg("-convert")
        .arg("json")
        .arg("-o")
        .arg("-")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn read_key_value(plist_json: &serde_json::Value, domain: &str, key: &str) -> Option<String> {
    if let Some(val) = plist_json.get(key) {
        return Some(format_plist_value(key, val));
    }

    let output = Command::new("/usr/bin/defaults")
        .arg("read")
        .arg(domain)
        .arg(key)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(redact_if_sensitive(key, raw))
}

fn format_plist_value(key: &str, value: &serde_json::Value) -> String {
    let rendered = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    };
    redact_if_sensitive(key, rendered)
}

fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == "inferenceGatewayApiKey" {
        return format!(
            "<present, {} chars>",
            raw.chars().filter(|c| !c.is_whitespace()).count()
        );
    }
    raw
}
