//! Hermes configuration probing via dotted-key YAML lookup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

pub(super) use crate::integration::config_read::DomainRead;

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
    Some(DomainRead::collect(
        source,
        KEYS_OF_INTEREST,
        |dotted| lookup_dotted(&value, dotted),
        |_, raw| raw,
    ))
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
    sysproc::find_processes("hermes")
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
