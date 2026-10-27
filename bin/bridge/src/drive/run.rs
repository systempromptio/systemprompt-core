//! The Drive sync loop and the one-shot cycle it repeats.
//!
//! A cycle pulls before it uploads, so a local edit is always committed on
//! top of the newest head this device knows of. Cycles are woken by the
//! watcher, a `document.revision.created` frame, an explicit "sync now" or the
//! [`TICK`], and serialised on the handle's cycle lock. Every failure is
//! logged and recorded as the status's `last_error`; `synced_at` moves only
//! when a whole cycle succeeded. A server with the document store disabled is
//! not a failure: the status turns `enabled = false` with no error and the
//! loop re-checks at its normal cadence. The status turns `enabled = true`
//! only once the server has answered a manifest or change-feed request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use systemprompt_identifiers::DriveDeviceId;

use super::conflict::prune_resolved;
use super::error::{DriveError, DriveResult};
use super::presence::{PresenceThrottle, document_for, send_editing};
use super::pull::{PullReport, pull};
use super::scan::local_changes;
use super::state::DriveState;
use super::status::DriveHandle;
use super::upload::{UploadReport, upload};
use super::{Cycle, DriveClient, DriveRoot, watcher};
use crate::config::SharedRuntimeConfig;
use crate::proxy::drive_signal::{DriveSignal, DriveSignalReceiver};
use crate::proxy::token_cache::TokenCache;

pub const TICK: Duration = Duration::from_secs(20);
const AUTH_THRESHOLD_SECS: u64 = 300;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CycleReport {
    pub pull: PullReport,
    pub upload: UploadReport,
    pub pending_uploads: usize,
}

pub async fn sync_once(cycle: Cycle<'_>, handle: &DriveHandle) -> DriveResult<CycleReport> {
    let _serialised = handle.cycle.lock().await;
    let state_path = cycle.root.state_path();
    let mut state = DriveState::load(&state_path)?;
    if prune_resolved(cycle.root, &mut state) {
        state.save(&state_path)?;
    }
    let pull = pull(cycle, &mut state).await?;
    mark_server_enabled(handle, cycle.root);
    let upload = upload(cycle, &mut state).await?;
    let pending_uploads = local_changes(cycle.root, &state)?.len();
    let report = CycleReport {
        pull,
        upload,
        pending_uploads,
    };
    handle.update(|status| {
        status.synced_at = Some(Utc::now());
        status.pending_uploads = report.pending_uploads;
        status.conflicts.clone_from(&state.conflicts);
        status.last_error = None;
    });
    Ok(report)
}

/// What the Drive loop runs against: the folder, the gateway and credential
/// the proxy already holds, and the signals the proxy forwards.
pub struct DriveDeps {
    pub handle: Arc<DriveHandle>,
    pub root: DriveRoot,
    pub runtime_config: SharedRuntimeConfig,
    pub token_cache: Arc<TokenCache>,
    pub http: reqwest::Client,
    pub device_id: DriveDeviceId,
    pub host_name: String,
    pub signals: DriveSignalReceiver,
}

impl std::fmt::Debug for DriveDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriveDeps")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

pub async fn run_loop(deps: DriveDeps) {
    let DriveDeps {
        handle,
        root,
        runtime_config,
        token_cache,
        http,
        device_id,
        host_name,
        signals,
    } = deps;
    let mut signals = Some(signals);
    let mut runner = Runner {
        handle: &handle,
        root: &root,
        runtime_config: &runtime_config,
        token_cache: &token_cache,
        http: &http,
        device_id: &device_id,
        host_name: &host_name,
        throttle: PresenceThrottle::default(),
    };
    let mut watch = None;
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let due = tokio::select! {
            _ = interval.tick() => true,
            () = handle.woken() => true,
            signal = next_signal(&mut signals) => signal.is_some_and(|s| runner.observe(s)),
        };
        if !due {
            continue;
        }
        if watch.is_none() && root.dir().is_dir() {
            watch = start_watcher(&root, &handle);
        }
        runner.cycle().await;
    }
}

async fn next_signal(signals: &mut Option<DriveSignalReceiver>) -> Option<DriveSignal> {
    let Some(receiver) = signals.as_mut() else {
        return std::future::pending().await;
    };
    let signal = receiver.recv().await;
    if signal.is_none() {
        tracing::debug!("drive signal channel closed; the loop polls only");
        *signals = None;
    }
    signal
}

fn start_watcher(root: &DriveRoot, handle: &Arc<DriveHandle>) -> Option<watcher::DriveWatcher> {
    match watcher::start(root.dir(), Arc::clone(handle)) {
        Ok(watch) => Some(watch),
        Err(error) => {
            tracing::warn!(%error, root = %root.dir().display(), "drive watcher could not start; local edits are found by the poll");
            None
        },
    }
}

struct Runner<'a> {
    handle: &'a Arc<DriveHandle>,
    root: &'a DriveRoot,
    runtime_config: &'a SharedRuntimeConfig,
    token_cache: &'a TokenCache,
    http: &'a reqwest::Client,
    device_id: &'a DriveDeviceId,
    host_name: &'a str,
    throttle: PresenceThrottle,
}

impl Runner<'_> {
    fn observe(&self, signal: DriveSignal) -> bool {
        match signal {
            DriveSignal::Touched(path) => {
                let ours = path.starts_with(self.root.dir());
                if ours {
                    self.handle.touch(path);
                }
                ours
            },
            DriveSignal::RevisionCreated(event) => {
                tracing::debug!(document_id = %event.document_id, "drive revision announced");
                true
            },
            DriveSignal::Presence(event) => {
                let path = match DriveState::load(&self.root.state_path()) {
                    Ok(state) => state.path_of(&event.document_id).map(str::to_owned),
                    Err(error) => {
                        tracing::warn!(%error, "drive presence shown without a path");
                        None
                    },
                };
                self.handle.observe_presence(&event, path);
                false
            },
        }
    }

    async fn cycle(&mut self) {
        if self.token_cache.sign_in_required() {
            return;
        }
        match self.try_cycle().await {
            Ok(()) => {},
            Err(DriveError::ServerDisabled { endpoint }) => {
                tracing::debug!(endpoint, "drive disabled on the server");
                mark_server_disabled(self.handle);
            },
            Err(error) => {
                if let DriveError::Unauthorized { endpoint } = &error {
                    self.token_cache.reject_upstream(endpoint).await;
                }
                tracing::warn!(%error, root = %self.root.dir().display(), "drive sync cycle failed");
                let message = error.to_string();
                self.handle
                    .update(|status| status.last_error = Some(message));
            },
        }
    }

    async fn try_cycle(&mut self) -> DriveResult<()> {
        let base = self
            .runtime_config
            .load_full()
            .gateway_base
            .as_ref()
            .clone();
        let client = DriveClient::new(base, self.http.clone());
        let token = self
            .token_cache
            .current(AUTH_THRESHOLD_SECS)
            .await
            .map_err(DriveError::Token)?;
        let cycle = Cycle {
            client: &client,
            bearer: &token.token,
            root: self.root,
            device_id: self.device_id,
            host_name: self.host_name,
        };
        if !ensure_enabled(cycle, self.handle).await? {
            return Ok(());
        }
        sync_once(cycle, self.handle).await?;
        self.beacon(cycle).await
    }

    async fn beacon(&mut self, cycle: Cycle<'_>) -> DriveResult<()> {
        let touched = self.handle.take_touched();
        if touched.is_empty() {
            return Ok(());
        }
        let state = DriveState::load(&self.root.state_path())?;
        let documents: Vec<_> = touched
            .iter()
            .filter_map(|path| document_for(self.root, &state, path))
            .collect();
        send_editing(cycle, &mut self.throttle, documents, Instant::now()).await?;
        Ok(())
    }
}

pub async fn ensure_enabled(cycle: Cycle<'_>, handle: &DriveHandle) -> DriveResult<bool> {
    let dir = cycle.root.dir();
    if !dir.is_dir() {
        let manifest = match cycle.client.manifest(cycle.bearer).await {
            Ok(manifest) => manifest,
            Err(DriveError::ServerDisabled { .. }) => {
                mark_server_disabled(handle);
                return Ok(false);
            },
            Err(error) => return Err(error),
        };
        if manifest.entries.is_empty() {
            return Ok(false);
        }
        std::fs::create_dir_all(dir).map_err(|e| DriveError::io("create Drive folder", dir, e))?;
        tracing::info!(root = %dir.display(), documents = manifest.entries.len(), "drive folder created");
        mark_server_enabled(handle, cycle.root);
    }
    Ok(true)
}

fn mark_server_enabled(handle: &DriveHandle, root: &DriveRoot) {
    handle.update(|status| {
        status.enabled = true;
        status.root = Some(root.dir().to_path_buf());
    });
}

pub fn mark_server_disabled(handle: &DriveHandle) {
    handle.update(|status| {
        status.enabled = false;
        status.last_error = None;
    });
}
