//! Per-plugin sync application: change detection and materialisation.
//!
//! Plugin files are fetched into a staging directory ([`super::fetch`]) that
//! only becomes the plugin on success, so a failure part-way leaves the
//! installed plugin untouched.
//!
//! Cancellation is cooperative and lands only between plugins: a plugin that
//! has started is either promoted or left as it was, never half-swapped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::fetch::fetch_plugin_into_staging;
use super::hooks::{PluginJsonShape, ensure_plugin_json_managed_fields, write_hooks_json};
use super::node_deps::{self, NodeInstall};
use super::swap::promote_staged;
use crate::auth::plugin_oauth::PluginTokenCache;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{HookEntry, PluginEntry, SignedManifest};
use crate::hash::safe_plugin_id;
use crate::host_sync::{HostWarning, HostWarnings};
use crate::ids::{BearerToken, HostId};
use crate::proxy::LoopbackEndpoint;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

pub(crate) struct PluginApplyOutcome {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
    pub host_failures: Vec<HostFailure>,
    pub host_warnings: Vec<HostWarning>,
    pub mcp_servers_by_plugin: BTreeMap<String, Vec<String>>,
    pub receipts: Vec<crate::fsutil::FileReceipt>,
}

pub(crate) enum PluginPhase {
    Complete(PluginApplyOutcome),
    Cancelled { applied: usize },
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostFailure {
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub host_id: HostId,
    pub emitter: String,
    pub error: String,
    pub needs_elevation: bool,
}

impl HostFailure {
    #[must_use]
    pub fn sentinel_line(&self) -> String {
        let first_line = self.error.lines().next().unwrap_or_default();
        format!("{}: {}: {first_line}", self.host_id, self.emitter)
    }
}

#[tracing::instrument(level = "debug", skip(ctx, manifest))]
pub(super) async fn apply_plugins(
    ctx: &PluginSyncCtx<'_>,
    manifest: &SignedManifest,
) -> Result<PluginPhase, super::ApplyError> {
    let mut installed = Vec::new();
    let mut updated = Vec::new();
    let mut malformed = Vec::new();
    let mut mcp_servers_by_plugin = BTreeMap::new();
    let mut receipts = Vec::new();
    let warnings = HostWarnings::new();
    let total = manifest.plugins.len();
    for (index, plugin) in manifest.plugins.iter().enumerate() {
        if ctx.cancel.is_cancelled() {
            return Ok(PluginPhase::Cancelled { applied: index });
        }
        if !safe_plugin_id(plugin.id.as_str()) {
            return Err(super::ApplyError::UnsafePluginId(plugin.id.clone()));
        }
        ctx.progress.report(&crate::progress::SyncProgress::new(
            "plugins",
            plugin.id.to_string(),
            index + 1,
            total,
        ));
        let applied = sync_one_plugin(ctx, plugin, &manifest.hooks).await?;
        receipts.push(applied.hooks_receipt);
        if let NodeInstall::Skipped { reason } = &applied.node_install {
            warnings.push(
                NODE_WARNING_HOST,
                format!(
                    "plugin {}: Node packages not installed — {reason}",
                    plugin.id
                ),
            );
        }
        match applied.change {
            PluginChange::Installed(id) => installed.push(id),
            PluginChange::Updated(id) => updated.push(id),
        }
        let plugin_dir = ctx.root.join(plugin.id.as_str());
        let servers = extract_mcp_servers(&plugin_dir)?;
        if !servers.is_empty() {
            mcp_servers_by_plugin.insert(plugin.id.to_string(), servers);
        }
        match applied.manifest_shape {
            PluginJsonShape::Stamped => {},
            PluginJsonShape::Absent => {
                tracing::warn!(
                    plugin_id = %plugin.id,
                    "synced plugin is missing claude-plugin/plugin.json — Claude Desktop will skip it"
                );
                malformed.push(plugin.id.to_string());
            },
            PluginJsonShape::Malformed(detail) => {
                tracing::warn!(
                    plugin_id = %plugin.id,
                    detail = %detail,
                    "synced plugin.json cannot be read; delivered verbatim and reported"
                );
                malformed.push(plugin.id.to_string());
            },
        }
    }
    if ctx.cancel.is_cancelled() {
        return Ok(PluginPhase::Cancelled { applied: total });
    }

    let expected: HashSet<&str> = manifest.plugins.iter().map(|p| p.id.as_str()).collect();
    let removed = remove_stale(ctx.root, &expected)?;
    for id in &removed {
        ctx.plugin_tokens
            .invalidate_plugin(&systemprompt_identifiers::PluginId::new(id));
    }

    Ok(PluginPhase::Complete(PluginApplyOutcome {
        installed,
        updated,
        removed,
        malformed,
        host_failures: Vec::new(),
        host_warnings: warnings.drain(),
        mcp_servers_by_plugin,
        receipts,
    }))
}

#[derive(serde::Deserialize)]
struct McpFileProbe {
    // JSON: `.mcp.json` is Claude Code's own file; only the server names are
    // read, the per-server bodies are opaque here.
    #[serde(rename = "mcpServers", default)]
    mcp_servers: BTreeMap<String, serde_json::Value>,
}

// Why: Claude Desktop would register a bundled `.mcp.json` verbatim, bypassing
// the loopback proxy; the names are recorded and the file is stripped only
// once it has parsed — a malformed file stays in place for the operator to see.
fn extract_mcp_servers(plugin_dir: &Path) -> Result<Vec<String>, super::ApplyError> {
    let path = plugin_dir.join(".mcp.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(super::ApplyError::Io {
                context: format!("read {}", path.display()),
                source,
            });
        },
    };
    let names: Vec<String> = serde_json::from_slice::<McpFileProbe>(&bytes)
        .map(|f| f.mcp_servers.into_keys().collect())
        .map_err(|e| super::ApplyError::Serialize {
            what: format!("{} is not valid JSON", path.display()),
            source: e,
        })?;
    crate::fsutil::remove_verified(&path).map_err(|source| super::ApplyError::Io {
        context: format!("strip bundled {}", path.display()),
        source,
    })?;
    Ok(names)
}

// Why: a Node install serves every host that reads the org-plugins tree, so
// its warning is filed under the tree rather than under one host.
const NODE_WARNING_HOST: &str = "org-plugins";

enum PluginChange {
    Installed(String),
    Updated(String),
}

struct PluginApplied {
    change: PluginChange,
    hooks_receipt: crate::fsutil::FileReceipt,
    manifest_shape: PluginJsonShape,
    node_install: NodeInstall,
}

pub(super) struct PluginSyncCtx<'a> {
    pub client: &'a GatewayClient,
    pub bearer: &'a BearerToken,
    pub loopback: &'a LoopbackEndpoint,
    pub plugin_tokens: &'a PluginTokenCache,
    pub root: &'a Path,
    pub staging_root: &'a Path,
    pub cancel: &'a CancellationToken,
    // Why: borrowing this sink across file-fetch awaits triggers rustc's higher-ranked Send error.
    pub progress: crate::progress::SyncProgressSink,
}

#[tracing::instrument(level = "debug", skip(ctx, plugin, hook_pool), fields(plugin_id = %plugin.id))]
async fn sync_one_plugin(
    ctx: &PluginSyncCtx<'_>,
    plugin: &PluginEntry,
    hook_pool: &[HookEntry],
) -> Result<PluginApplied, super::ApplyError> {
    let target = ctx.root.join(plugin.id.as_str());

    let stage = ctx.staging_root.join(plugin.id.as_str());
    fetch_plugin_into_staging(ctx.client, ctx.bearer, plugin, &stage).await?;
    super::check_not_superseded(ctx.client.base_url())?;

    let was_present = promote_staged(&stage, &target, plugin.id.as_str())?;

    let hooks_receipt = write_hooks_json(ctx.loopback, plugin, &target, hook_pool)?;
    let manifest_shape = ensure_plugin_json_managed_fields(&target)?;
    let install_dir = target.clone();
    let node_install = tokio::task::spawn_blocking(move || node_deps::install(&install_dir))
        .await
        .map_err(|error| super::ApplyError::Io {
            context: format!("run the Node install for {}", plugin.id),
            source: std::io::Error::other(error),
        })?;
    if let NodeInstall::Installed { tool } = &node_install {
        tracing::info!(
            target: "bridge::sync::node",
            plugin_id = %plugin.id,
            tool,
            "installed the plugin's Node packages from its lockfile"
        );
    }

    let change = if was_present {
        PluginChange::Updated(plugin.id.to_string())
    } else {
        PluginChange::Installed(plugin.id.to_string())
    };
    Ok(PluginApplied {
        change,
        hooks_receipt,
        manifest_shape,
        node_install,
    })
}

fn remove_stale(root: &Path, expected: &HashSet<&str>) -> Result<Vec<String>, super::ApplyError> {
    let mut removed = Vec::new();
    let entries = fs::read_dir(root).map_err(|source| super::ApplyError::Io {
        context: format!("enumerate {}", root.display()),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| super::ApplyError::Io {
            context: format!("enumerate {}", root.display()),
            source,
        })?;
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
