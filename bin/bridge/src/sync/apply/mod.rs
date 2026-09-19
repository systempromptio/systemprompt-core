//! Applies a verified manifest to disk: plugins, hooks, MCP fragments.
//!
//! A run belongs to the gateway it fetched from. Before anything is
//! published the configured gateway is re-read from disk — the GUI rewrites
//! the config while a sync is in flight — and a run whose gateway is no
//! longer the configured one is refused as superseded, so it never promotes a
//! plugin or publishes a registry the new gateway did not deliver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod evidence;
mod fetch;
mod hooks;
pub(super) mod loopback;
pub mod node_deps;
mod plugin;
pub mod safe_path;
pub mod swap;

pub(crate) use crate::host_sync::ApplyError;
pub use crate::host_sync::{HostWarning, HostWarningKind};

pub use plugin::HostFailure;

pub const PLUGIN_INSTALLATION_PREFERENCE: &str = "required";

use crate::config::paths::{self, OrgPluginsLocation};
use crate::context::BridgeContext;
use crate::fsutil::{FileReceipt, atomic_write_0644};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{ManagedMcpServer, SignedManifest, UserInfo};
use crate::host_sync::{self, HostSyncCtx};
use crate::ids::{BearerToken, HostId};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::ValidatedUrl;
use tokio_util::sync::CancellationToken;

pub(crate) use plugin::PluginApplyOutcome as ApplyReport;

pub(crate) enum ApplyOutcome {
    Applied(ApplyReport),
    Cancelled { applied: usize },
}

pub(crate) struct ApplyRequest<'a> {
    pub client: &'a GatewayClient,
    pub bearer: &'a BearerToken,
    pub bridge: &'a BridgeContext,
    pub manifest: &'a SignedManifest,
    pub location: &'a OrgPluginsLocation,
    pub cancel: &'a CancellationToken,
}

pub(crate) async fn apply_manifest(req: &ApplyRequest<'_>) -> Result<ApplyOutcome, ApplyError> {
    let ApplyRequest {
        client,
        bearer,
        bridge,
        manifest,
        location,
        cancel,
    } = *req;
    let _installation_lock = tokio::select! {
        () = cancel.cancelled() => return Ok(ApplyOutcome::Cancelled { applied: 0 }),
        lock = crate::feedback::installation_lock() => lock.map_err(|error| ApplyError::Io {
            context: "serialize native host installation".to_owned(),
            source: std::io::Error::other(error),
        })?,
    };
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
        cancel,
        progress: bridge.sync_progress.clone(),
    };
    let phase = plugin::apply_plugins(&plugin_ctx, manifest).await;
    crate::fsutil::remove_leftover_dir(&staging_root);
    let mut report = match phase? {
        plugin::PluginPhase::Complete(report) => report,
        plugin::PluginPhase::Cancelled { applied } => {
            return Ok(ApplyOutcome::Cancelled { applied });
        },
    };

    check_not_superseded(client.base_url())?;

    let mcp_servers =
        loopback::rewrite_loopback_urls(&manifest.managed_mcp_servers, client.base_url());
    let manifest_for_write = loopback::manifest_with_servers(manifest, mcp_servers.clone());
    report
        .receipts
        .push(write_user(&meta_dir, manifest.user.as_ref())?);
    report.receipts.push(write_mcp_servers(
        &meta_dir,
        client.base_url(),
        &mcp_servers,
    )?);

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
        start_menu: &bridge.start_menu,
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
                host_id: HostId::new(host_id),
                emitter: emitter.emitter_id().to_owned(),
                error: format!("{e:#}"),
                needs_elevation: matches!(e, ApplyError::ElevationRequired { .. }),
            });
        }
        host_sync::log_outcome(*emitter, enabled, outcome);
    }
    evidence::capture(emitters, &manifest_for_write, &ctx, &warnings, &mut report).await;
    report.host_warnings.extend(warnings.drain());

    Ok(ApplyOutcome::Applied(report))
}

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

pub fn metadata_dir() -> Result<std::path::PathBuf, ApplyError> {
    let meta_dir = paths::bridge_metadata_dir().ok_or_else(|| ApplyError::Io {
        context: "resolve bridge metadata dir".into(),
        source: std::io::Error::other("no LOCALAPPDATA / state dir resolvable"),
    })?;
    fs::create_dir_all(&meta_dir).map_err(|e| ApplyError::Io {
        context: format!("create metadata dir at {}", meta_dir.display()),
        source: e,
    })?;
    Ok(meta_dir)
}

pub fn prepare_dirs(root: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), ApplyError> {
    fs::create_dir_all(root).map_err(|e| ApplyError::Io {
        context: format!("create {}", root.display()),
        source: e,
    })?;
    let meta_dir = metadata_dir()?;
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

pub fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<FileReceipt, ApplyError> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u).map_err(|e| ApplyError::Serialize {
            what: "user".into(),
            source: e,
        })?,
        None => b"null".to_vec(),
    };
    write_fragment(&path, &bytes)
}

// Why: the Windows policy writer accepts only a gateway-signed manifest and
// re-verifies it under SYSTEM, so the envelope is kept verbatim rather than
// the decoded manifest — a decoded copy could not be re-verified.
pub fn write_envelope(
    meta_dir: &Path,
    gateway: &ValidatedUrl,
    envelope: &systemprompt_models::bridge::manifest::SignedManifestEnvelope,
) -> Result<FileReceipt, ApplyError> {
    let path = meta_dir.join(paths::MANIFEST_ENVELOPE_FRAGMENT);
    let fragment = crate::mcp_registry::EnvelopeFragment {
        gateway: gateway.clone(),
        envelope: envelope.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&fragment).map_err(|e| ApplyError::Serialize {
        what: "manifest envelope".into(),
        source: e,
    })?;
    write_fragment(&path, &bytes)
}

pub fn write_mcp_servers(
    meta_dir: &Path,
    gateway: &ValidatedUrl,
    servers: &[ManagedMcpServer],
) -> Result<FileReceipt, ApplyError> {
    let path = meta_dir.join(paths::MCP_SERVERS_FRAGMENT);
    let fragment = crate::mcp_registry::McpServersFragment {
        gateway: gateway.clone(),
        servers: servers.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&fragment).map_err(|e| ApplyError::Serialize {
        what: "managed MCP servers".into(),
        source: e,
    })?;
    write_fragment(&path, &bytes)
}

fn write_fragment(path: &Path, bytes: &[u8]) -> Result<FileReceipt, ApplyError> {
    atomic_write_0644(path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })?;
    FileReceipt::verify(path, bytes).map_err(|e| ApplyError::Io {
        context: format!("verify {}", path.display()),
        source: e,
    })
}
