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

use super::hooks::{PluginJsonShape, ensure_plugin_json_managed_fields, write_hooks_json};
use super::node_deps::{self, NodeInstall};
use super::plugin_files::{extract_mcp_servers, remove_stale};
use super::stage::{Staged, stage_changed};
use super::stamp::{self, PluginStamp};
use super::swap::promote_staged;
use crate::auth::plugin_oauth::PluginTokenCache;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{HookEntry, PluginEntry, SignedManifest};
use crate::hash::safe_plugin_id;
use crate::host_sync::{HostWarning, HostWarningKind, WarningScope};
use crate::ids::BearerToken;
use crate::proxy::LoopbackEndpoint;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use systemprompt_models::bridge::host::HostKind;
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
    pub host_id: HostKind,
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
    let mut warnings = Vec::new();
    let total = manifest.plugins.len();
    if let Some(unsafe_id) = manifest
        .plugins
        .iter()
        .find(|p| !safe_plugin_id(p.id.as_str()))
    {
        return Err(super::ApplyError::UnsafePluginId(unsafe_id.id.clone()));
    }
    let unchanged: HashMap<&str, PluginStamp> = manifest
        .plugins
        .iter()
        .filter_map(|p| stamp::current(ctx.meta_dir, ctx.root, p).map(|s| (p.id.as_str(), s)))
        .collect();
    let changed: Vec<&PluginEntry> = manifest
        .plugins
        .iter()
        .filter(|p| !unchanged.contains_key(p.id.as_str()))
        .collect();
    tracing::info!(
        target: "bridge::sync",
        unchanged = unchanged.len(),
        changed = changed.len(),
        "plugins compared against their installed stamps"
    );
    if let Staged::Cancelled = stage_changed(ctx, &changed, unchanged.len(), total).await? {
        return Ok(PluginPhase::Cancelled { applied: 0 });
    }
    for (index, plugin) in manifest.plugins.iter().enumerate() {
        if ctx.cancel.is_cancelled() {
            return Ok(PluginPhase::Cancelled { applied: index });
        }
        let applied = match unchanged.get(plugin.id.as_str()) {
            Some(stamp) => refresh_unchanged(ctx, plugin, &manifest.hooks, stamp).await?,
            None => promote_plugin(ctx, plugin, &manifest.hooks).await?,
        };
        receipts.push(applied.hooks_receipt);
        if let NodeInstall::Skipped { reason } = &applied.node_install {
            warnings.push(HostWarning::raise(
                HostWarningKind::NodePackages,
                WarningScope::OrgPlugins,
                format!(
                    "plugin {}: Node packages not installed — {reason}",
                    plugin.id
                ),
            ));
        }
        match applied.change {
            PluginChange::Installed(id) => installed.push(id),
            PluginChange::Updated(id) => updated.push(id),
            PluginChange::Unchanged => {},
        }
        if !applied.mcp_servers.is_empty() {
            mcp_servers_by_plugin.insert(plugin.id.to_string(), applied.mcp_servers);
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
        stamp::clear(ctx.meta_dir, id)?;
        ctx.plugin_tokens
            .invalidate_plugin(&systemprompt_identifiers::PluginId::new(id));
    }

    Ok(PluginPhase::Complete(PluginApplyOutcome {
        installed,
        updated,
        removed,
        malformed,
        host_failures: Vec::new(),
        host_warnings: warnings,
        mcp_servers_by_plugin,
        receipts,
    }))
}

enum PluginChange {
    Installed(String),
    Updated(String),
    Unchanged,
}

struct PluginApplied {
    change: PluginChange,
    hooks_receipt: crate::fsutil::FileReceipt,
    manifest_shape: PluginJsonShape,
    node_install: NodeInstall,
    mcp_servers: Vec<String>,
}

pub(super) struct PluginSyncCtx<'a> {
    pub client: &'a GatewayClient,
    pub bearer: &'a BearerToken,
    pub loopback: &'a LoopbackEndpoint,
    pub plugin_tokens: &'a PluginTokenCache,
    pub root: &'a Path,
    pub meta_dir: &'a Path,
    pub staging_root: &'a Path,
    pub cancel: &'a CancellationToken,
    // Why: borrowing this sink across file-fetch awaits triggers rustc's higher-ranked Send error.
    pub progress: crate::progress::SyncProgressSink,
}

// Why: hooks.json embeds the loopback endpoint, which can move between runs,
// so an unchanged plugin still has its hooks and plugin.json fields restamped;
// the Node pass is its own no-op unless its lockfile fingerprint changed.
async fn refresh_unchanged(
    ctx: &PluginSyncCtx<'_>,
    plugin: &PluginEntry,
    hook_pool: &[HookEntry],
    stamp: &PluginStamp,
) -> Result<PluginApplied, super::ApplyError> {
    let target = ctx.root.join(plugin.id.as_str());
    let hooks_receipt = write_hooks_json(ctx.loopback, plugin, &target, hook_pool)?;
    let manifest_shape = ensure_plugin_json_managed_fields(&target)?;
    Ok(PluginApplied {
        change: PluginChange::Unchanged,
        hooks_receipt,
        manifest_shape,
        node_install: install_node_packages(plugin, &target).await?,
        mcp_servers: stamp.mcp_servers.clone(),
    })
}

#[tracing::instrument(level = "debug", skip(ctx, plugin, hook_pool), fields(plugin_id = %plugin.id))]
async fn promote_plugin(
    ctx: &PluginSyncCtx<'_>,
    plugin: &PluginEntry,
    hook_pool: &[HookEntry],
) -> Result<PluginApplied, super::ApplyError> {
    let target = ctx.root.join(plugin.id.as_str());
    let stage = ctx.staging_root.join(plugin.id.as_str());
    super::check_not_superseded(ctx.client.base_url())?;

    stamp::clear(ctx.meta_dir, plugin.id.as_str())?;
    let was_present = promote_staged(&stage, &target, &plugin.id)?;

    let hooks_receipt = write_hooks_json(ctx.loopback, plugin, &target, hook_pool)?;
    let manifest_shape = ensure_plugin_json_managed_fields(&target)?;
    let node_install = install_node_packages(plugin, &target).await?;
    let mcp_servers = extract_mcp_servers(&target)?;
    stamp::write(
        ctx.meta_dir,
        plugin.id.as_str(),
        &PluginStamp {
            sha256: plugin.sha256.as_str().to_owned(),
            root: ctx.root.to_path_buf(),
            mcp_servers: mcp_servers.clone(),
        },
    )?;

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
        mcp_servers,
    })
}

async fn install_node_packages(
    plugin: &PluginEntry,
    target: &Path,
) -> Result<NodeInstall, super::ApplyError> {
    let install_dir = target.to_path_buf();
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
    Ok(node_install)
}
