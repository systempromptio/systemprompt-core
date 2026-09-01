//! `OpenCode` configuration probing over the managed tier.
//!
//! Only the managed sources count: a `provider.systemprompt` block in a user
//! or project file is not governance, and reporting it as installed would hide
//! that the managed tier is missing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::Value;

use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
}

// Why: managed preferences are binary plists; `plutil` is the only reader
// guaranteed present, and it renders the whole document as JSON in one call.
#[cfg(target_os = "macos")]
fn read_macos_managed() -> Option<DomainRead> {
    for path in config::macos_managed_prefs_paths() {
        if !path.exists() {
            continue;
        }
        let out = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&path)
            .output()
            .ok()?;
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(read) = parse_into_keys(&text, &path.display().to_string()) {
            return Some(read);
        }
    }
    None
}

pub(super) fn read_config() -> DomainRead {
    #[cfg(target_os = "macos")]
    if let Some(read) = read_macos_managed() {
        return read;
    }
    let managed = config::managed_config_path();
    if managed.exists()
        && let Ok(text) = std::fs::read_to_string(&managed)
        && let Some(read) = parse_into_keys(&text, &managed.display().to_string())
    {
        return read;
    }
    let jsonc = config::managed_jsonc_path();
    if jsonc.exists() {
        // Why: the bridge never writes `.jsonc` and does not parse comments; a
        // managed `.jsonc` is foreign config, reported as the source but never
        // read.
        tracing::warn!(
            source = %jsonc.display(),
            "opencode probe: managed opencode.jsonc present; bridge reads opencode.json only"
        );
        return DomainRead {
            source_path: Some(jsonc.display().to_string()),
            keys: BTreeMap::new(),
        };
    }
    DomainRead::default()
}

fn parse_into_keys(text: &str, source: &str) -> Option<DomainRead> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: Value = serde_json::from_str(text)
        .map_err(|e| {
            tracing::warn!(error = %e, source = %source, "opencode probe: JSON parse failed");
        })
        .ok()?;
    let mut out = DomainRead {
        source_path: Some(source.to_owned()),
        keys: BTreeMap::new(),
    };
    for dotted in KEYS_OF_INTEREST {
        if let Some(raw) = lookup_dotted(&value, dotted) {
            out.keys.insert((*dotted).to_owned(), raw);
        }
    }
    Some(out)
}

// Why: model ids carry dots (`gpt-4.1`), so the `models` object is displayed
// as its sorted key list rather than addressed through them.
fn lookup_dotted(root: &Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    Some(stringify(cur))
}

fn stringify(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            keys.join(", ")
        },
        Value::Bool(_) | Value::Number(_) | Value::Null | Value::Array(_) => v.to_string(),
    }
}

pub(super) fn list_opencode_processes() -> Vec<String> {
    let mut hits: Vec<String> = sysproc::list_processes()
        .into_iter()
        .filter_map(|p| {
            let name_lower = p.name.to_ascii_lowercase();
            let path_lower = p
                .path
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if cfg!(target_os = "windows") {
                if name_lower == "opencode.exe" {
                    return Some(name_lower);
                }
            } else if path_lower.ends_with("/opencode")
                || path_lower.contains("/opencode.app/")
                || name_lower == "opencode"
            {
                return Some(if path_lower.is_empty() {
                    name_lower
                } else {
                    path_lower
                });
            }
            None
        })
        .collect();
    hits.sort();
    hits.dedup();
    hits
}
