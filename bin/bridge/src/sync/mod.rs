//! Plugin/MCP sync pipeline: fetch, verify, hash-compare, and apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod acceptance;
pub mod apply;
mod attempt;
mod error;
mod manifest;
mod manifest_cache;
mod org_plugins_scope;
mod process_lock;
mod provision;
mod registry_refresh;
mod replay;
mod seed_model;
mod sentinel;
mod stale_retry;
mod summary;
mod unsafe_flags;

use self::attempt::run_attempt;
pub use crate::last_sync::{
    LastSyncState, ReplayStateError, last_synced_auto_update_policy, last_synced_enabled_hosts,
    read_last_sync,
};
pub use apply::{HostFailure, HostWarning, HostWarningKind, PLUGIN_INSTALLATION_PREFERENCE};
pub use error::{CredentialRejection, SyncError};
pub use provision::ProvisionError;
pub use registry_refresh::{refresh_registry, refresh_registry_for};
pub use replay::{SKEW_WINDOW_MINUTES, check_replay, check_skew};
pub use stale_retry::run_once;
pub use summary::SyncSummary;
use summary::build_summary;
pub use unsafe_flags::warn_unsafe_flags;

use crate::config;
use crate::gateway::Freshness;

pub const WATCH_FLOOR_SECS: u64 = 60;

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub allow_unsigned: bool,
    pub force_replay: bool,
    pub allow_tofu: bool,
    pub freshness: Freshness,
    pub cancel: tokio_util::sync::CancellationToken,
}

// Why: the credential is only attribution; a sync must never fail because
// the gateway declined to enrol this install.
async fn ensure_device_enrolled(
    bridge: &crate::context::BridgeContext,
    fetch: &manifest::ManifestFetch,
    user_id: &systemprompt_identifiers::UserId,
) {
    let Ok(root) = crate::feedback::metadata_root() else {
        return;
    };
    if crate::feedback::credentials::Enrollment::load(&root, fetch.client.base_url_str()).is_ok() {
        return;
    }
    let Some(install_id) = bridge.durable_install_id() else {
        tracing::warn!(
            "install identity is not durable; device self-enrolment skipped so a throwaway \
             identity is never enrolled"
        );
        return;
    };
    let enrolment = crate::feedback::enrol::SelfEnrolment {
        install_id: install_id.as_str(),
        user_id,
        label: crate::sysproc::host_name(),
        force_rotate: false,
    };
    if let Err(error) =
        crate::feedback::enrol::ensure_self_enrolled(&fetch.client, &fetch.bearer, &enrolment).await
    {
        tracing::warn!(%error, "device self-enrolment failed; installation feedback stays unattributed");
    }
}

fn prepare_run(bridge: &crate::context::BridgeContext) -> Result<(), SyncError> {
    bridge
        .activity
        .ensure_persistence()
        .map_err(|source| SyncError::Persistence {
            path: crate::obs::log_dir()
                .unwrap_or_default()
                .join("activity.jsonl"),
            source,
        })?;
    bridge
        .sync_progress
        .report(&crate::progress::SyncProgress::new(
            "manifest", "manifest", 1, 1,
        ));
    Ok(())
}

// Why: a headless sync (the OS scheduler, `sync --watch`) has no proxy
// heartbeat to retry receipts, so sync does it; it runs before the sync locks
// so a slow gateway never holds them, and the outbox's own lock and backoff
// make it safe beside the heartbeat's retry.
async fn retry_pending_feedback() {
    if let Ok(config) = config::load() {
        let gateway = config::gateway_url_or_default(&config);
        if let Err(error) = crate::feedback::retry_pending(gateway.as_str()).await {
            tracing::debug!(%error, "Installation receipts remain unacknowledged before sync");
        }
    }
}

fn persist_envelope(fetch: &manifest::ManifestFetch) -> Result<(), SyncError> {
    let meta_dir = apply::metadata_dir().map_err(|e| SyncError::ApplyFailed(Box::new(e)))?;
    apply::write_envelope(
        &meta_dir,
        fetch.client.base_url(),
        &fetch.envelope,
        fetch.etag.as_deref(),
    )
    .map(|_| ())
    .map_err(|e| SyncError::ApplyFailed(Box::new(e)))
}

fn prior_checkpoint(
    last_sync_path: &std::path::Path,
    run_gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<LastSyncState, SyncError> {
    match read_last_sync(last_sync_path) {
        // Why: manifest versions are per gateway; the previous gateway's
        // version says nothing about whether this one is a replay.
        Ok(Some(s)) if s.belongs_to(run_gateway) => Ok(s),
        Ok(_) => Ok(LastSyncState::default()),
        Err(e) => {
            tracing::error!(error = %e, "replay state file is corrupt; refusing to apply");
            Err(SyncError::from(e))
        },
    }
}

fn ensure_not_superseded(
    run_gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<(), SyncError> {
    apply::check_not_superseded(run_gateway).map_err(apply_error_to_sync)
}

fn apply_error_to_sync(e: apply::ApplyError) -> SyncError {
    match e {
        apply::ApplyError::Superseded {
            started_for,
            current,
        } => SyncError::Superseded {
            started_for,
            current,
        },
        other => SyncError::ApplyFailed(Box::new(other)),
    }
}
