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

pub mod archive;
mod evidence;
mod fetch;
mod hooks;
pub(super) mod loopback;
pub mod node_deps;
mod plugin;
mod plugin_files;
pub mod safe_path;
mod stage;
pub mod stamp;
pub mod swap;
mod fragments;
pub mod skill_bodies;

pub(crate) use crate::host_sync::ApplyError;
pub use crate::host_sync::{HostWarning, HostWarningKind};

pub use fragments::{write_envelope, write_mcp_servers, write_user};
pub use plugin::HostFailure;

pub const PLUGIN_INSTALLATION_PREFERENCE: &str = "required";

use crate::config::paths::{self, OrgPluginsLocation};
use crate::context::BridgeContext;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::{self, HostSyncCtx};
use crate::ids::BearerToken;
use std::fs;
use std::path::Path;
use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::bridge::host::HostKind;
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
        meta_dir: &meta_dir,
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
    let mut manifest_for_write = loopback::manifest_with_servers(manifest, mcp_servers.clone());
    skill_bodies::resolve(&mut manifest_for_write, root);
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
    let ctx = HostSyncCtx {
        policy_store: &bridge.policy_store,
        manifest: &manifest_for_write,
        org_plugins_root: root,
        plugin_mcp_servers: &plugin_mcp_servers,
        client,
        bearer,
        loopback,
        mcp_registry: &registry,
        start_menu: &bridge.start_menu,
    };
    for (skill, host) in crate::gateway::manifest::unknown_skill_hosts(&manifest_for_write) {
        tracing::warn!(
            target: "bridge::sync::host",
            skill,
            host,
            "skill targets a host id this bridge does not know; no host receives it under that id"
        );
    }
    let emitters = host_sync::registry();
    let mut skipped_absent = std::collections::BTreeSet::new();
    for (index, emitter) in emitters.iter().enumerate() {
        let host_id = emitter.host_id();
        bridge
            .sync_progress
            .report(&crate::progress::SyncProgress::new(
                "hosts",
                host_id.as_str().to_owned(),
                index + 1,
                emitters.len(),
            ));
        let enabled = crate::gateway::manifest::enables_host(&manifest_for_write, host_id);
        if enabled && app_absent(host_id, &bridge.start_menu) {
            tracing::info!(
                host = host_id.as_str(),
                "host app not installed; settings not written"
            );
            skipped_absent.insert(host_id);
            continue;
        }
        let outcome = if enabled {
            emitter.apply(&ctx).await
        } else {
            emitter
                .clear(&ctx)
                .map(|()| host_sync::HostSyncReport::ok())
        };
        match outcome {
            Ok(emitted) => {
                report.host_warnings.extend(emitted.warnings);
                host_sync::log_outcome(*emitter, enabled, Ok(()));
            },
            Err(e) => {
                host_sync::log_outcome(*emitter, enabled, Err(&e));
                report.host_failures.push(HostFailure {
                    host_id,
                    emitter: emitter.emitter_id().to_owned(),
                    error: format!("{e:#}"),
                    needs_elevation: matches!(e, ApplyError::ElevationRequired { .. }),
                });
            },
        }
    }
    let evidence_warnings = evidence::capture(
        emitters,
        &manifest_for_write,
        &ctx,
        &skipped_absent,
        &mut report,
    )
    .await;
    report.host_warnings.extend(evidence_warnings);

    Ok(ApplyOutcome::Applied(report))
}

fn app_absent(host_id: HostKind, start_menu: &crate::probe_cache::StartMenuCache) -> bool {
    crate::integration::find_host_by_id(host_id).is_some_and(|host| {
        host.app_installed(start_menu) == crate::integration::AppInstallState::NotInstalled
    })
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
