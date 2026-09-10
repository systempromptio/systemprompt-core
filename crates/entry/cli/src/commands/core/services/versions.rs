//! Content-versus-version drift between a services tree and its predecessor.
//!
//! A consumer pins a plugin by version, so a plugin whose files changed while
//! its `version` stayed put ships different content under a name the consumer
//! already believes it has. The comparison is per-plugin and uses the previous
//! bundle's own manifest checksums, so it sees exactly the bytes that were
//! published rather than whatever is in the working tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use systemprompt_models::services::bundle::ServicesBundleManifest;

pub const PLUGIN_CONFIG_FILE: &str = "config.yaml";

#[derive(Debug, Deserialize)]
struct PluginVersionFile {
    plugin: PluginVersion,
}

#[derive(Debug, Deserialize)]
struct PluginVersion {
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDrift {
    pub plugin: String,
    pub version: String,
    pub changed_files: usize,
}

pub fn detect_drift(
    previous: &ServicesBundleManifest,
    previous_root: &Path,
    current: &ServicesBundleManifest,
    current_root: &Path,
) -> Result<Vec<VersionDrift>> {
    let previous_files = plugin_files(previous);
    let current_files = plugin_files(current);

    let mut drift = Vec::new();
    for (plugin, before) in &previous_files {
        let Some(after) = current_files.get(plugin) else {
            continue;
        };
        let changed = changed_count(before, after);
        if changed == 0 {
            continue;
        }
        let old_version = read_version(previous_root, plugin)?;
        let new_version = read_version(current_root, plugin)?;
        if old_version == new_version {
            drift.push(VersionDrift {
                plugin: plugin.clone(),
                version: new_version.unwrap_or_else(|| "unknown".to_owned()),
                changed_files: changed,
            });
        }
    }
    Ok(drift)
}

fn changed_count(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> usize {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .count()
}

fn plugin_files(manifest: &ServicesBundleManifest) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut grouped: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for entry in &manifest.files {
        let Some(rest) = entry.path.strip_prefix("plugins/") else {
            continue;
        };
        let Some((plugin, relative)) = rest.split_once('/') else {
            continue;
        };
        grouped
            .entry(plugin.to_owned())
            .or_default()
            .insert(relative.to_owned(), entry.sha256.clone());
    }
    grouped
}

fn read_version(root: &Path, plugin: &str) -> Result<Option<String>> {
    let path = root.join("plugins").join(plugin).join(PLUGIN_CONFIG_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let parsed: PluginVersionFile = serde_yaml::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(parsed.plugin.version))
}
