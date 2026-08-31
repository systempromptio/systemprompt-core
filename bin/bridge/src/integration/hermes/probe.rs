//! Hermes configuration probing via dotted-key YAML lookup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
}

pub(super) fn read_config() -> DomainRead {
    let path = config::config_yaml_path();
    if path.exists()
        && let Ok(text) = std::fs::read_to_string(&path)
        && let Some(read) = parse_into_keys(&text, &path.display().to_string())
    {
        return read;
    }
    DomainRead::default()
}

fn parse_into_keys(text: &str, source: &str) -> Option<DomainRead> {
    let value: serde_yaml::Value = serde_yaml::from_str(text)
        .map_err(|e| {
            tracing::warn!(error = %e, source = %source, "hermes probe: YAML parse failed");
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

fn lookup_dotted(root: &serde_yaml::Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        let key = segment.trim_matches('"');
        cur = cur
            .as_mapping()?
            .get(serde_yaml::Value::String(key.to_owned()))?;
    }
    Some(stringify(cur))
}

fn stringify(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => "null".to_owned(),
        serde_yaml::Value::Sequence(_)
        | serde_yaml::Value::Mapping(_)
        | serde_yaml::Value::Tagged(_) => serde_yaml::to_string(v)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    }
}

pub(super) fn list_hermes_processes() -> Vec<String> {
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
                if name_lower == "hermes.exe" {
                    return Some(name_lower);
                }
            } else if path_lower.ends_with("/hermes")
                || path_lower.contains("/hermes.app/")
                || name_lower == "hermes"
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

// Why: mirrors the Codex probe's `write_dotted`, walking/creating nested
// mappings so a bridge-owned key can be set without disturbing siblings.
pub(super) fn write_dotted(
    target: &mut serde_yaml::Value,
    dotted: &str,
    value: serde_yaml::Value,
) -> bool {
    let segments: Vec<&str> = dotted.split('.').collect();
    let mut cur = target;
    for segment in &segments[..segments.len() - 1] {
        let key = serde_yaml::Value::String(segment.trim_matches('"').to_owned());
        let serde_yaml::Value::Mapping(map) = cur else {
            return false;
        };
        let entry = map
            .entry(key)
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        if !matches!(entry, serde_yaml::Value::Mapping(_)) {
            *entry = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
        }
        cur = entry;
    }
    let last = serde_yaml::Value::String(segments[segments.len() - 1].trim_matches('"').to_owned());
    if let serde_yaml::Value::Mapping(m) = cur {
        m.insert(last, value);
        true
    } else {
        false
    }
}
