use super::super::hash::{directory_hash, normalise_relative, safe_plugin_id, sha256_hex};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{PluginEntry, SignedManifest};
use crate::ids::Sha256Digest;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct PluginApplyOutcome {
    /// Pre-rendered plugin-id strings for display in sync reports; the typed PluginId has been
    /// consumed by this point.
    pub installed: Vec<String>,
    /// See `installed`.
    pub updated: Vec<String>,
    /// See `installed`.
    pub removed: Vec<String>,
    /// See `installed`.
    pub malformed: Vec<String>,
}

pub fn apply_plugins(
    client: &GatewayClient,
    bearer: &str,
    manifest: &SignedManifest,
    root: &Path,
    staging_root: &Path,
) -> Result<PluginApplyOutcome, super::ApplyError> {
    let mut installed = Vec::new();
    let mut updated = Vec::new();
    let mut malformed = Vec::new();

    for plugin in &manifest.plugins {
        if !safe_plugin_id(plugin.id.as_str()) {
            return Err(super::ApplyError::Detail(format!(
                "manifest contained unsafe plugin id: {}",
                plugin.id
            )));
        }
        if let Some(change) = sync_one_plugin(client, bearer, plugin, root, staging_root)? {
            match change {
                PluginChange::Installed(id) => installed.push(id),
                PluginChange::Updated(id) => updated.push(id),
            }
        }
        if !is_well_formed(&root.join(plugin.id.as_str())) {
            tracing::warn!(
                plugin_id = %plugin.id,
                "synced plugin is missing claude-plugin/plugin.json — Claude Desktop will skip it"
            );
            malformed.push(plugin.id.to_string());
        }
    }

    let expected: HashSet<&str> = manifest.plugins.iter().map(|p| p.id.as_str()).collect();
    let removed = remove_stale(root, &expected)?;

    Ok(PluginApplyOutcome {
        installed,
        updated,
        removed,
        malformed,
    })
}

fn is_well_formed(plugin_dir: &Path) -> bool {
    plugin_dir
        .join("claude-plugin")
        .join("plugin.json")
        .is_file()
}

enum PluginChange {
    Installed(String),
    Updated(String),
}

fn sync_one_plugin(
    client: &GatewayClient,
    bearer: &str,
    plugin: &PluginEntry,
    root: &Path,
    staging_root: &Path,
) -> Result<Option<PluginChange>, super::ApplyError> {
    let target = root.join(plugin.id.as_str());
    let current_hash = target
        .is_dir()
        .then(|| directory_hash(&target).ok())
        .flatten();
    if current_hash.as_deref() == Some(plugin.sha256.as_str()) {
        return Ok(None);
    }

    let stage = staging_root.join(plugin.id.as_str());
    fetch_plugin_into_staging(client, bearer, plugin, &stage)?;

    let staged_hash = directory_hash(&stage)
        .map_err(|e| super::ApplyError::Detail(format!("hash staged {}: {e}", plugin.id)))?;
    if staged_hash != plugin.sha256.as_str() {
        return Err(super::ApplyError::Detail(format!(
            "plugin {} hash mismatch (expected {}, got {})",
            plugin.id, plugin.sha256, staged_hash
        )));
    }

    let was_present = target.exists();
    if was_present {
        fs::remove_dir_all(&target)
            .map_err(|e| super::ApplyError::Detail(format!("remove old {}: {e}", plugin.id)))?;
    }
    fs::rename(&stage, &target).map_err(|e| {
        super::ApplyError::Detail(format!("rename stage→target for {}: {e}", plugin.id))
    })?;

    Ok(Some(if was_present {
        PluginChange::Updated(plugin.id.to_string())
    } else {
        PluginChange::Installed(plugin.id.to_string())
    }))
}

fn fetch_plugin_into_staging(
    client: &GatewayClient,
    bearer: &str,
    plugin: &PluginEntry,
    stage: &Path,
) -> Result<(), super::ApplyError> {
    fs::create_dir_all(stage)
        .map_err(|e| super::ApplyError::Detail(format!("create stage {}: {e}", stage.display())))?;
    for file in &plugin.files {
        if file.path.contains("..") || file.path.starts_with('/') || file.path.starts_with('\\') {
            return Err(super::ApplyError::Detail(format!(
                "unsafe path in manifest: {}",
                file.path
            )));
        }
        let out = stage.join(normalise_relative(&file.path));
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                super::ApplyError::Detail(format!("create parent {}: {e}", parent.display()))
            })?;
        }
        let bytes = client
            .fetch_plugin_file(bearer, plugin.id.as_str(), &file.path)
            .map_err(|e| e.to_string())?;
        let actual = sha256_hex(&bytes);
        if !sha256_matches(&actual, &file.sha256) {
            return Err(super::ApplyError::Detail(format!(
                "file {}/{} hash mismatch (expected {}, got {})",
                plugin.id, file.path, file.sha256, actual
            )));
        }
        fs::write(&out, &bytes)
            .map_err(|e| super::ApplyError::Detail(format!("write {}: {e}", out.display())))?;
    }
    Ok(())
}

fn sha256_matches(actual: &str, expected: &Sha256Digest) -> bool {
    actual == expected.as_str()
}

fn remove_stale(root: &Path, expected: &HashSet<&str>) -> Result<Vec<String>, super::ApplyError> {
    let mut removed = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(removed);
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') {
            continue;
        }
        if !expected.contains(name_str) && entry.path().is_dir() {
            fs::remove_dir_all(entry.path())
                .map_err(|e| super::ApplyError::Detail(format!("remove stale {name_str}: {e}")))?;
            removed.push(name_str.to_string());
        }
    }
    Ok(removed)
}
