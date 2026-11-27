//! The background loop of one Drive sync: when cycles run, the sync token they
//! use, and how their outcome becomes the published status.
//!
//! A cycle runs at start, on every poll, on a debounced change in the folder
//! and on demand; cycles never overlap. The sync token is traded for the
//! sign-in's device token and reused until shortly before it expires; a refused
//! device token is reported to its source. An instance without a document store
//! is asked again only after [`super::SyncOptions::disabled_recheck`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use chrono::Utc;
use sp_client::DeviceTokenSource;
use sp_protocol::Jwt;
use sp_protocol::ids::DriveDeviceId;
use sp_protocol::wire::drive::SyncTokenAnswer;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use super::Shared;
use super::cycle::{self, Cycle, Report, Summary};
use super::error::SyncError;
use super::feed::{self, FeedClient};
use super::folder_watch::{self, FolderWatch};
use super::root::SyncRoot;
use super::status::{SyncPhase, SyncProblem, SyncProblemKind};

const SYNC_TOKEN_MARGIN_SECS: i64 = 60;

pub(super) struct Runner {
    pub(super) shared: Arc<Shared>,
    pub(super) feed: FeedClient,
    pub(super) credentials: Arc<dyn DeviceTokenSource>,
    pub(super) root: SyncRoot,
    pub(super) device_id: DriveDeviceId,
    pub(super) host: String,
    pub(super) sync_token: Option<SyncTokenAnswer>,
    pub(super) disabled_at: Option<Instant>,
    pub(super) watch: Option<FolderWatch>,
}

impl Runner {
    pub(super) async fn run(mut self, cancel: CancellationToken) {
        let shared = Arc::clone(&self.shared);
        let mut interval = tokio::time::interval(shared.options.poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = interval.tick() => {},
                () = shared.wake.notified() => {},
            }
            if shared.paused.load(Ordering::Acquire) {
                continue;
            }
            if let Some(at) = self.disabled_at
                && at.elapsed() < shared.options.disabled_recheck
            {
                continue;
            }
            self.disabled_at = None;
            if self.watch.is_none() {
                self.watch = match folder_watch::start(
                    self.root.dir(),
                    shared.options.debounce,
                    Arc::clone(&shared.wake),
                ) {
                    Ok(watch) => Some(watch),
                    Err(error) => {
                        tracing::warn!(error = %error, "the Drive folder watch did not start; the poll finds local edits");
                        None
                    },
                };
            }
            tokio::select! {
                () = cancel.cancelled() => break,
                () = self.cycle() => {},
            }
        }
        tracing::debug!("the Drive sync loop ended");
    }

    async fn cycle(&mut self) {
        let shared = Arc::clone(&self.shared);
        let _serial = shared.cycle.lock().await;
        shared
            .status
            .send_modify(|status| status.phase = SyncPhase::Syncing);
        let mut report = Report::default();
        let outcome = self.try_cycle(&mut report).await;
        let keep = shared.options.recent_items;
        let paused = shared.paused.load(Ordering::Acquire);
        let idle = if paused {
            SyncPhase::Paused
        } else {
            SyncPhase::Idle
        };
        match outcome {
            Ok(summary) => {
                let problem = report.refused.first().map(|(path, reason)| SyncProblem {
                    kind: SyncProblemKind::Refused,
                    message: format!("{path} was not uploaded: {reason}"),
                    at: Utc::now(),
                });
                shared.status.send_modify(|status| {
                    status.phase = idle;
                    status.last_synced = Some(Utc::now());
                    status.absorb(summary);
                    status.record(report.activity, keep);
                    status.problem = problem;
                });
            },
            Err(SyncError::Disabled { endpoint }) => {
                tracing::debug!(
                    endpoint,
                    "the instance has no document store; the Drive sync waits"
                );
                self.disabled_at = Some(Instant::now());
                shared.status.send_modify(|status| {
                    status.phase = SyncPhase::Disabled;
                    status.problem = None;
                    status.record(report.activity, keep);
                });
            },
            Err(error) => {
                if error.refuses_token() {
                    self.sync_token = None;
                }
                tracing::warn!(error = %error, root = %self.root.dir().display(), "a Drive sync cycle failed");
                let problem = SyncProblem {
                    kind: problem_kind(&error),
                    message: error.to_string(),
                    at: Utc::now(),
                };
                shared.status.send_modify(|status| {
                    status.phase = SyncPhase::Error;
                    status.problem = Some(problem);
                    status.record(report.activity, keep);
                });
            },
        }
    }

    async fn try_cycle(&mut self, report: &mut Report) -> Result<Summary, SyncError> {
        let token = self.sync_token().await?;
        let cx = Cycle {
            feed: &self.feed,
            token: &token,
            root: &self.root,
            device_id: &self.device_id,
            host: &self.host,
        };
        cycle::run(cx, report).await
    }

    async fn sync_token(&mut self) -> Result<Jwt, SyncError> {
        if let Some(current) = &self.sync_token
            && current.expires_at - chrono::Duration::seconds(SYNC_TOKEN_MARGIN_SECS) > Utc::now()
        {
            return Ok(current.token.clone());
        }
        self.sync_token = None;
        let device = self
            .credentials
            .device_token()
            .await
            .map_err(SyncError::Credential)?
            .ok_or(SyncError::DeviceTokenMissing)?;
        match self.feed.sync_token(&device).await {
            Ok(issued) => {
                let token = issued.token.clone();
                self.sync_token = Some(issued);
                Ok(token)
            },
            Err(error) => {
                if error.refuses_token() {
                    tracing::warn!(
                        endpoint = feed::TOKEN,
                        "the instance refused the device token"
                    );
                    self.credentials.device_token_refused().await;
                }
                Err(error)
            },
        }
    }
}

const fn problem_kind(error: &SyncError) -> SyncProblemKind {
    match error {
        SyncError::Client(client) if client.refuses_token() => SyncProblemKind::SignedOut,
        SyncError::Credential(_) => SyncProblemKind::SignedOut,
        SyncError::DeviceTokenMissing => SyncProblemKind::NoDeviceToken,
        SyncError::Client(client) if client.is_transient() => SyncProblemKind::Unreachable,
        SyncError::Rejected { .. } => SyncProblemKind::Refused,
        SyncError::Io { .. }
        | SyncError::Secure(_)
        | SyncError::IndexCorrupt { .. }
        | SyncError::UnsafePath { .. }
        | SyncError::Unplaceable { .. }
        | SyncError::RootTaken { .. }
        | SyncError::InvalidRoot { .. } => SyncProblemKind::LocalFiles,
        SyncError::Encode(_)
        | SyncError::HashMismatch { .. }
        | SyncError::IncompleteChange { .. }
        | SyncError::HeadMissing { .. }
        | SyncError::Disabled { .. }
        | SyncError::Client(_) => SyncProblemKind::Failed,
    }
}
