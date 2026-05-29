use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::{PluginEntry, PluginFile};
use systemprompt_models::services::{PluginConfig, ServicesConfig};

use crate::error::MarketplaceError;

const PLUGIN_BLOCKED_FILENAMES: &[&str] = &["config.yaml", "config.yml"];

#[must_use]
pub fn load_plugins(services_root: &Path, services: &ServicesConfig) -> Vec<PluginEntry> {
    let plugins_root = services_root.join("plugins");
    let mut configs: Vec<&PluginConfig> = services.plugins.values().filter(|p| p.enabled).collect();
    configs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    let mut out = Vec::with_capacity(configs.len());
    for config in configs {
        match build_plugin_entry(&plugins_root, config) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    "manifest: plugin directory missing on disk; skipping"
                );
            },
            Err(e) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    error = %e,
                    "manifest: failed to build plugin entry; skipping"
                );
            },
        }
    }
    out
}

fn build_plugin_entry(
    plugins_root: &Path,
    config: &PluginConfig,
) -> Result<Option<PluginEntry>, MarketplaceError> {
    let plugin_dir = plugins_root.join(config.id.as_str());
    if !plugin_dir.is_dir() {
        return Ok(None);
    }

    let mut files: BTreeMap<String, PluginFile> = BTreeMap::new();
    collect_files(&plugin_dir, &plugin_dir, &mut files)?;
    let mut hasher = Sha256::new();
    hasher.update(config.id.as_str().as_bytes());
    hasher.update(config.version.as_bytes());
    for file in files.values() {
        hasher.update(file.path.as_bytes());
        hasher.update(file.sha256.as_str().as_bytes());
    }
    let aggregate = hex::encode(hasher.finalize());
    let sha256 =
        Sha256Digest::try_new(aggregate).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    let id = PluginId::try_new(config.id.as_str())
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    Ok(Some(PluginEntry {
        id,
        version: config.version.clone(),
        sha256,
        files: files.into_values().collect(),
    }))
}

fn collect_files(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, PluginFile>,
) -> Result<(), MarketplaceError> {
    let read = std::fs::read_dir(dir).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    for entry in read {
        let entry = entry.map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
            if PLUGIN_BLOCKED_FILENAMES.contains(&name) {
                continue;
            }
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let Some(rel_str) = rel.to_str() else {
            continue;
        };
        let normalized = rel_str.replace('\\', "/");
        let bytes = std::fs::read(&path).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        let size = bytes.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hex::encode(hasher.finalize());
        let sha256 =
            Sha256Digest::try_new(digest).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        out.insert(
            normalized.clone(),
            PluginFile {
                path: normalized,
                sha256,
                size,
            },
        );
    }
    Ok(())
}
