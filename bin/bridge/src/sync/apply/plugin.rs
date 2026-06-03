use super::super::hash::{normalise_relative, safe_plugin_id, sha256_hex};
use super::hooks::{ensure_plugin_json_hooks_field, write_hooks_json};
use crate::auth::plugin_oauth::global_cache;
use crate::config::paths;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{HookEntry, PluginEntry, SignedManifest};
use crate::ids::Sha256Digest;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(crate) struct PluginApplyOutcome {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
    pub host_failures: Vec<HostFailure>,
}

#[derive(Debug, Clone)]
pub struct HostFailure {
    pub host_id: String,
    pub error: String,
}

#[tracing::instrument(level = "debug", skip(client, bearer, manifest))]
pub(super) async fn apply_plugins(
    client: &GatewayClient,
    bearer: &str,
    manifest: &SignedManifest,
    root: &Path,
    staging_root: &Path,
) -> Result<PluginApplyOutcome, super::ApplyError> {
    let mut installed = Vec::new();
    let mut updated = Vec::new();
    let mut malformed = Vec::new();

    let ctx = PluginSyncCtx {
        client,
        bearer,
        root,
        staging_root,
    };
    for plugin in &manifest.plugins {
        if !safe_plugin_id(plugin.id.as_str()) {
            return Err(super::ApplyError::UnsafePluginId(plugin.id.clone()));
        }
        match sync_one_plugin(&ctx, plugin, &manifest.hooks).await? {
            PluginChange::Installed(id) => installed.push(id),
            PluginChange::Updated(id) => updated.push(id),
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
    if !removed.is_empty() {
        let cache = global_cache().await;
        for id in &removed {
            cache.invalidate(&systemprompt_identifiers::PluginId::new(id));
        }
    }

    Ok(PluginApplyOutcome {
        installed,
        updated,
        removed,
        malformed,
        host_failures: Vec::new(),
    })
}

fn is_well_formed(plugin_dir: &Path) -> bool {
    super::plugin_manifest_path(plugin_dir).is_some()
}

enum PluginChange {
    Installed(String),
    Updated(String),
}

struct PluginSyncCtx<'a> {
    client: &'a GatewayClient,
    bearer: &'a str,
    root: &'a Path,
    staging_root: &'a Path,
}

#[tracing::instrument(level = "debug", skip(ctx, plugin, user_hooks), fields(plugin_id = %plugin.id))]
async fn sync_one_plugin(
    ctx: &PluginSyncCtx<'_>,
    plugin: &PluginEntry,
    user_hooks: &[HookEntry],
) -> Result<PluginChange, super::ApplyError> {
    let target = ctx.root.join(plugin.id.as_str());

    let stage = ctx.staging_root.join(plugin.id.as_str());
    fetch_plugin_into_staging(ctx.client, ctx.bearer, plugin, &stage).await?;

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

    let plugin_id_typed = systemprompt_identifiers::PluginId::new(plugin.id.as_str());
    write_hooks_json(&plugin_id_typed, &target, user_hooks)?;
    ensure_plugin_json_hooks_field(&target)?;

    Ok(if was_present {
        PluginChange::Updated(plugin.id.to_string())
    } else {
        PluginChange::Installed(plugin.id.to_string())
    })
}

async fn fetch_plugin_into_staging(
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
        let bytes = client
            .fetch_plugin_file(bearer, plugin.id.as_str(), &file.path)
            .await?;
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
        if name_str == paths::SYNTHETIC_PLUGIN_NAME {
            continue;
        }
        if !expected.contains(name_str) && entry.path().is_dir() {
            fs::remove_dir_all(entry.path()).map_err(|e| super::ApplyError::Io {
                context: format!("remove stale {name_str}"),
                source: e,
            })?;
            removed.push(name_str.to_owned());
        }
    }
    Ok(removed)
}
