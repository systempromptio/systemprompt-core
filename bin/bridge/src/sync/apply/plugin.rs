//! Per-plugin sync application: change detection and materialisation.
//!
//! Plugin files are fetched serially rather than concurrently. A buffered
//! variant of the fetch loop tips rustc's "Send is not general enough" limit:
//! awaiting the resulting stream deep inside the sync chain leaves borrows
//! (`&GatewayClient`, the `&str` bearer) held across the await, and the spawned
//! sync task then fails to prove `Send` for all lifetimes. Serial keeps those
//! borrows out of a combinator's higher-ranked bound. Staging is into a
//! temporary directory that only becomes the plugin on success either way, so a
//! failure part-way leaves the installed plugin untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::hooks::{ensure_plugin_json_managed_fields, write_hooks_json};
use crate::auth::plugin_oauth::PluginTokenCache;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{HookEntry, PluginEntry, SignedManifest};
use crate::hash::{normalise_relative, safe_plugin_id, sha256_hex};
use crate::ids::Sha256Digest;
use crate::proxy::LoopbackEndpoint;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

pub(crate) struct PluginApplyOutcome {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
    pub host_failures: Vec<HostFailure>,
    pub mcp_servers_by_plugin: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostFailure {
    pub host_id: String,
    pub error: String,
}

#[tracing::instrument(level = "debug", skip(ctx, manifest))]
pub(super) async fn apply_plugins(
    ctx: &PluginSyncCtx<'_>,
    manifest: &SignedManifest,
) -> Result<PluginApplyOutcome, super::ApplyError> {
    let mut installed = Vec::new();
    let mut updated = Vec::new();
    let mut malformed = Vec::new();
    let mut mcp_servers_by_plugin = BTreeMap::new();
    let total = manifest.plugins.len();
    for (index, plugin) in manifest.plugins.iter().enumerate() {
        if !safe_plugin_id(plugin.id.as_str()) {
            return Err(super::ApplyError::UnsafePluginId(plugin.id.clone()));
        }
        // Why: this loop is the wall time. Every plugin is fetched file by file,
        // one request at a time, so it is the only place where "still working"
        // can be distinguished from "stuck".
        ctx.progress
            .report(&crate::sync::progress::SyncProgress::new(
                "plugins",
                plugin.id.to_string(),
                index + 1,
                total,
            ));
        match sync_one_plugin(ctx, plugin, &manifest.hooks).await? {
            PluginChange::Installed(id) => installed.push(id),
            PluginChange::Updated(id) => updated.push(id),
        }
        let servers = extract_mcp_servers(&ctx.root.join(plugin.id.as_str()));
        if !servers.is_empty() {
            mcp_servers_by_plugin.insert(plugin.id.to_string(), servers);
        }
        if !is_well_formed(&ctx.root.join(plugin.id.as_str())) {
            tracing::warn!(
                plugin_id = %plugin.id,
                "synced plugin is missing claude-plugin/plugin.json — Claude Desktop will skip it"
            );
            malformed.push(plugin.id.to_string());
        }
    }

    let expected: HashSet<&str> = manifest.plugins.iter().map(|p| p.id.as_str()).collect();
    let removed = remove_stale(ctx.root, &expected)?;
    if !removed.is_empty() {
        for id in &removed {
            ctx.plugin_tokens
                .invalidate_plugin(&systemprompt_identifiers::PluginId::new(id));
        }
    }

    Ok(PluginApplyOutcome {
        installed,
        updated,
        removed,
        malformed,
        host_failures: Vec::new(),
        mcp_servers_by_plugin,
    })
}

#[derive(serde::Deserialize)]
struct McpFileProbe {
    #[serde(rename = "mcpServers", default)]
    mcp_servers: BTreeMap<String, serde_json::Value>,
}

fn extract_mcp_servers(plugin_dir: &Path) -> Vec<String> {
    let path = plugin_dir.join(".mcp.json");
    let Ok(bytes) = fs::read(&path) else {
        return Vec::new();
    };
    let names = serde_json::from_slice::<McpFileProbe>(&bytes)
        .map(|f| f.mcp_servers.into_keys().collect())
        .unwrap_or_default();
    if let Err(e) = fs::remove_file(&path) {
        tracing::warn!(
            target: "bridge::sync",
            path = %path.display(),
            error = %e,
            "could not strip bundled .mcp.json"
        );
    }
    names
}

fn is_well_formed(plugin_dir: &Path) -> bool {
    super::plugin_manifest_path(plugin_dir).is_some()
}

enum PluginChange {
    Installed(String),
    Updated(String),
}

pub(super) struct PluginSyncCtx<'a> {
    pub client: &'a GatewayClient,
    pub bearer: &'a str,
    pub loopback: &'a LoopbackEndpoint,
    pub plugin_tokens: &'a PluginTokenCache,
    pub root: &'a Path,
    pub staging_root: &'a Path,
    // Why: owned (cheap Arc-backed clone), not borrowed — holding a `&SyncProgressSink`
    // in this ctx across the per-file fetch awaits added a borrow that tipped
    // rustc's "Send is not general enough" limit on the spawned sync task.
    pub progress: crate::sync::progress::SyncProgressSink,
}

#[tracing::instrument(level = "debug", skip(ctx, plugin, hook_pool), fields(plugin_id = %plugin.id))]
async fn sync_one_plugin(
    ctx: &PluginSyncCtx<'_>,
    plugin: &PluginEntry,
    hook_pool: &[HookEntry],
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

    write_hooks_json(ctx.loopback, plugin, &target, hook_pool)?;
    ensure_plugin_json_managed_fields(&target)?;

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
    // Why: every path is validated before a single request goes out. Doing this in
    // the fetch loop below would mean an unsafe path in the middle of a plugin
    // is only caught after its earlier files are already on disk.
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
    }

    // Why: serial, not concurrent — a buffered variant leaves borrows held
    // across the await and the spawned sync task then fails to prove `Send`
    // for all lifetimes. See the module head.
    for file in &plugin.files {
        let out = stage.join(normalise_relative(&file.path));
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
