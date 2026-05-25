//! Pure JSON manipulation for the two registry files Cowork reads:
//! `known_marketplaces.json` and `installed_plugins.json`.
//!
//! Each file is a JSON object with one known array key (`marketplaces`,
//! `installedPlugins`). All other keys are foreign and preserved verbatim.
//! Foreign entries inside the array (other marketplaces / other installed
//! plugins) are preserved; entries we own are upserted by name (or by the
//! `(marketplace, name)` pair for installed plugins).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::CoworkPluginsError;

const KEY_MARKETPLACES: &str = "marketplaces";
const KEY_INSTALLED: &str = "installedPlugins";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSource {
    #[serde(rename = "type")]
    pub kind: String,
    pub path: String,
}

impl LocalSource {
    #[must_use]
    pub fn local(path: String) -> Self {
        Self {
            kind: "local".into(),
            path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownMarketplaceEntry {
    pub name: String,
    pub source: LocalSource,
    #[serde(rename = "installedAt", skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<String>,
}

// Visibility is governed by `cowork_settings.json::enabledPlugins`, not by a
// field on this struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledPluginEntry {
    pub marketplace: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "installedAt", skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub inserted: Vec<String>,
    pub replaced: Vec<String>,
    pub unchanged: Vec<String>,
}

pub fn parse_root(bytes: &[u8]) -> Result<Map<String, Value>, CoworkPluginsError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(bytes)? {
        Value::Object(root) => Ok(root),
        _ => Err(CoworkPluginsError::RootShape),
    }
}

// Foreign entries (other marketplaces) MUST be preserved — Cowork users may
// have registered their own marketplaces alongside ours.
pub fn upsert_known_marketplace(
    root: &mut Map<String, Value>,
    entry: &KnownMarketplaceEntry,
) -> Result<MergeReport, CoworkPluginsError> {
    let new_value = serde_json::to_value(entry)?;
    upsert_by_name(root, KEY_MARKETPLACES, &entry.name, new_value)
}

// Identity is the `(marketplace, name)` pair, not name alone — same plugin
// name can exist in multiple marketplaces. Foreign entries preserved.
pub fn upsert_installed_plugin(
    root: &mut Map<String, Value>,
    entry: &InstalledPluginEntry,
) -> Result<MergeReport, CoworkPluginsError> {
    let next_value = serde_json::to_value(entry)?;
    let array = ensure_array(root, KEY_INSTALLED)?;

    let target_index = array.iter().position(|v| {
        matches_str(v, "marketplace", &entry.marketplace) && matches_str(v, "name", &entry.name)
    });

    let key = format!("{}::{}", entry.marketplace, entry.name);
    let mut report = MergeReport::default();
    if let Some(i) = target_index {
        if array[i] == next_value {
            report.unchanged.push(key);
        } else {
            array[i] = next_value;
            report.replaced.push(key);
        }
    } else {
        array.push(next_value);
        report.inserted.push(key);
    }
    Ok(report)
}

fn upsert_by_name(
    root: &mut Map<String, Value>,
    items_key: &'static str,
    name: &str,
    new_value: Value,
) -> Result<MergeReport, CoworkPluginsError> {
    let array = ensure_array(root, items_key)?;
    let mut report = MergeReport::default();
    if let Some(existing) = array.iter_mut().find(|v| matches_str(v, "name", name)) {
        if existing == &new_value {
            report.unchanged.push(name.to_string());
        } else {
            *existing = new_value;
            report.replaced.push(name.to_string());
        }
    } else {
        array.push(new_value);
        report.inserted.push(name.to_string());
    }
    Ok(report)
}

fn ensure_array<'a>(
    root: &'a mut Map<String, Value>,
    key: &'static str,
) -> Result<&'a mut Vec<Value>, CoworkPluginsError> {
    match root
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
    {
        Value::Array(v) => Ok(v),
        _ => Err(CoworkPluginsError::ItemsShape { key }),
    }
}

fn matches_str(v: &Value, key: &str, expected: &str) -> bool {
    v.as_object()
        .and_then(|o| o.get(key))
        .and_then(Value::as_str)
        == Some(expected)
}
