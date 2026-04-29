use super::super::hash::{directory_hash, normalise_relative, safe_plugin_id, sha256_hex};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{PluginEntry, SignedManifest};
use crate::ids::Sha256Digest;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct PluginApplyOutcome {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
}

#[tracing::instrument(level = "debug", skip(client, bearer, manifest))]
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
            return Err(super::ApplyError::UnsafePluginId(plugin.id.clone()));
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

#[tracing::instrument(level = "debug", skip(client, bearer, plugin), fields(plugin_id = %plugin.id))]
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

    let staged_hash = directory_hash(&stage).map_err(|e| super::ApplyError::Io {
        context: format!("hash staged {}", plugin.id),
        source: e,
    })?;
    if staged_hash != plugin.sha256.as_str() {
        return Err(super::ApplyError::HashMismatch {
            what: format!("plugin {}", plugin.id),
            expected: plugin.sha256.clone(),
            actual: staged_hash,
        });
    }

    let was_present = target.exists();
    if was_present {
        fs::remove_dir_all(&target).map_err(|e| super::ApplyError::Io {
            context: format!("remove old {}", plugin.id),
            source: e,
        })?;
    }
    fs::rename(&stage, &target).map_err(|e| super::ApplyError::Io {
        context: format!("rename stage→target for {}", plugin.id),
        source: e,
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
    fs::create_dir_all(stage).map_err(|e| super::ApplyError::Io {
        context: format!("create stage {}", stage.display()),
        source: e,
    })?;
    for file in &plugin.files {
        if file.path.contains("..") || file.path.starts_with('/') || file.path.starts_with('\\') {
            return Err(super::ApplyError::UnsafePath(file.path.clone()));
        }
        let out = stage.join(normalise_relative(&file.path));
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| super::ApplyError::Io {
                context: format!("create parent {}", parent.display()),
                source: e,
            })?;
        }
        let bytes = client.fetch_plugin_file(bearer, plugin.id.as_str(), &file.path)?;
        let actual = sha256_hex(&bytes);
        if !sha256_matches(&actual, &file.sha256) {
            return Err(super::ApplyError::HashMismatch {
                what: format!("file {}/{}", plugin.id, file.path),
                expected: file.sha256.clone(),
                actual,
            });
        }
        fs::write(&out, &bytes).map_err(|e| super::ApplyError::Io {
            context: format!("write {}", out.display()),
            source: e,
        })?;
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
            fs::remove_dir_all(entry.path()).map_err(|e| super::ApplyError::Io {
                context: format!("remove stale {name_str}"),
                source: e,
            })?;
            removed.push(name_str.to_string());
        }
    }
    Ok(removed)
}
