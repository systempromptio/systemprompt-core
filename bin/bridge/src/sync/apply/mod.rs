//! Applies a verified manifest to disk: plugins, hooks, MCP fragments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod hooks;
pub(crate) mod hooks_schema;
mod loopback;
mod plugin;

pub(crate) use crate::host_sync::ApplyError;
pub use plugin::{HostFailure, HostWarning};

pub const PLUGIN_INSTALLATION_PREFERENCE: &str = "required";

const LEGACY_SYNTHETIC_PLUGIN: &str = "systemprompt-managed";

use crate::config::paths::{self, OrgPluginsLocation};
use crate::context::BridgeContext;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{ManagedMcpServer, SignedManifest, UserInfo};
use crate::host_sync::{self, HostSyncCtx};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::ValidatedUrl;

pub(crate) use plugin::PluginApplyOutcome as ApplyReport;

pub(crate) async fn apply_manifest(
    client: &GatewayClient,
    bearer: &str,
    bridge: &BridgeContext,
    manifest: &SignedManifest,
    location: &OrgPluginsLocation,
) -> Result<ApplyReport, ApplyError> {
    let loopback = bridge.proxy.loopback();
    let root = &location.path;
    let (meta_dir, staging_root) = prepare_dirs(root)?;

    let plugin_ctx = plugin::PluginSyncCtx {
        client,
        bearer,
        loopback,
        plugin_tokens: &bridge.plugin_tokens,
        root,
        staging_root: &staging_root,
        progress: bridge.sync_progress.clone(),
    };
    let mut report = plugin::apply_plugins(&plugin_ctx, manifest).await?;

    crate::fsutil::remove_leftover_dir(&staging_root);
    check_not_superseded(client.base_url())?;
    prune_legacy_state();

    let mcp_servers =
        loopback::rewrite_loopback_urls(&manifest.managed_mcp_servers, client.base_url());
    let manifest_for_write = loopback::manifest_with_servers(manifest, mcp_servers.clone());
    write_user(&meta_dir, manifest.user.as_ref())?;
    write_mcp_servers(&meta_dir, client.base_url(), &mcp_servers)?;

    crate::mcp_registry::publish(&bridge.mcp_registry, &mcp_servers);
    let registry = bridge.mcp_registry();

    let plugin_mcp_servers = report.mcp_servers_by_plugin.clone();
    let warnings = host_sync::HostWarnings::new();
    let ctx = HostSyncCtx {
        policy_store: &bridge.policy_store,
        warnings: &warnings,
        manifest: &manifest_for_write,
        org_plugins_root: root,
        plugin_mcp_servers: &plugin_mcp_servers,
        client,
        bearer,
        loopback,
        mcp_registry: &registry,
    };
    let emitters = host_sync::registry();
    for (index, emitter) in emitters.iter().enumerate() {
        let host_id = emitter.host_id();
        bridge
            .sync_progress
            .report(&crate::progress::SyncProgress::new(
                "hosts",
                host_id.to_owned(),
                index + 1,
                emitters.len(),
            ));
        let enabled = manifest_for_write
            .enabled_hosts
            .iter()
            .any(|h| h == host_id);
        let outcome = if enabled {
            emitter.apply(&ctx).await
        } else {
            emitter.clear(&ctx)
        };
        if let Err(e) = &outcome {
            report.host_failures.push(HostFailure {
                host_id: host_id.to_owned(),
                error: format!("{e:#}"),
            });
        }
        host_sync::log_outcome(host_id, enabled, outcome);
    }
    report.host_warnings = warnings.drain();

    Ok(report)
}

/// Refuses to publish state from a run whose gateway is no longer the
/// configured one. Read from disk each time: the GUI rewrites the config
/// while a sync is in flight, and the run must notice before it promotes a
/// plugin or publishes a registry the new gateway never delivered.
pub(crate) fn check_not_superseded(run_gateway: &ValidatedUrl) -> Result<(), ApplyError> {
    let cfg = crate::config::load().map_err(|e| ApplyError::Io {
        context: "re-read gateway before publishing sync".into(),
        source: std::io::Error::other(e),
    })?;
    let current = crate::config::gateway_url_or_default(&cfg);
    if crate::mcp_registry::same_origin(run_gateway, &current) {
        return Ok(());
    }
    Err(ApplyError::Superseded {
        started_for: run_gateway.to_string(),
        current: current.to_string(),
    })
}

fn prune_legacy_state() {
    for root in paths::all_known_org_plugins_roots() {
        remove_legacy_dir(
            &root.join(LEGACY_SYNTHETIC_PLUGIN),
            "legacy aggregate plugin",
        );
        for marker in paths::LEGACY_ORG_PLUGINS_METADATA {
            remove_legacy_dir(&root.join(marker), "legacy bridge metadata dir");
        }
    }
}

fn remove_legacy_dir(path: &Path, what: &str) {
    if !path.exists() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(()) => tracing::info!(
            target: "bridge::sync",
            path = %path.display(),
            kind = what,
            "pruned legacy state"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::sync",
            path = %path.display(),
            kind = what,
            error = %e,
            "could not prune legacy state (likely permissions); skipping"
        ),
    }
}

pub fn prepare_dirs(root: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), ApplyError> {
    fs::create_dir_all(root).map_err(|e| ApplyError::Io {
        context: format!("create {}", root.display()),
        source: e,
    })?;
    let meta_dir = paths::bridge_metadata_dir().ok_or_else(|| ApplyError::Io {
        context: "resolve bridge metadata dir".into(),
        source: std::io::Error::other("no LOCALAPPDATA / state dir resolvable"),
    })?;
    fs::create_dir_all(&meta_dir).map_err(|e| ApplyError::Io {
        context: format!("create metadata dir at {}", meta_dir.display()),
        source: e,
    })?;
    let staging_root = paths::bridge_staging_dir().ok_or_else(|| ApplyError::Io {
        context: "resolve bridge staging dir".into(),
        source: std::io::Error::other("no LOCALAPPDATA / state dir resolvable"),
    })?;
    crate::fsutil::remove_leftover_dir(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| ApplyError::Io {
        context: format!("create staging at {}", staging_root.display()),
        source: e,
    })?;
    Ok((meta_dir, staging_root))
}

fn plugin_manifest_path(plugin_dir: &Path) -> Option<std::path::PathBuf> {
    use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_DIRS, PLUGIN_MANIFEST_FILE};
    PLUGIN_MANIFEST_DIRS
        .iter()
        .map(|dir| plugin_dir.join(dir).join(PLUGIN_MANIFEST_FILE))
        .find(|path| path.is_file())
}

pub fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<(), ApplyError> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u).map_err(|e| ApplyError::Serialize {
            what: "user".into(),
            source: e,
        })?,
        None => b"null".to_vec(),
    };
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

pub fn write_mcp_servers(
    meta_dir: &Path,
    gateway: &ValidatedUrl,
    servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    let path = meta_dir.join(paths::MCP_SERVERS_FRAGMENT);
    let fragment = crate::mcp_registry::McpServersFragment {
        gateway: gateway.clone(),
        servers: servers.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&fragment).map_err(|e| ApplyError::Serialize {
        what: "managed MCP servers".into(),
        source: e,
    })?;
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}
