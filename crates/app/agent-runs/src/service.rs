//! [`ClaimLoop`], the claim-and-dispatch loop, and [`DbEventSink`].
//!
//! An extension's `RunnerService` runs the loop; it claims queued runs with
//! `FOR UPDATE SKIP LOCKED` and hands each to a [`RunHandler`].
//! On shutdown the loop cancels in-flight runs and waits for their handlers.
//! `claimer_id` formats the conventional `<hostname>:<pid>` claimer.
//! [`DbEventSink`] is the repository-backed [`EventSink`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_identifiers::{HeadlessRunId, HostSessionId};
use systemprompt_traits::OwnedTask;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::error::HeadlessResult;
use crate::event::RunEvent;
use crate::execute::EventSink;
use crate::lease::{DEFAULT_LEASE, DEFAULT_RENEW_EVERY, LeaseRenewer, LeaseSpec};
use crate::repository::{HeadlessRunRepository, HeadlessRunRow};

/// A run the loop claimed. `run_token` is cancelled on shutdown or when the
/// lease is lost.
#[derive(Debug, Clone)]
pub struct ClaimedRun {
    pub row: HeadlessRunRow,
    pub run_token: CancellationToken,
    pub repo: HeadlessRunRepository,
}

/// Runs one claimed run to its end: calls `execute` and `finish`.
///
/// Held as `Arc<dyn RunHandler>` so the consumer picks the behaviour at
/// runtime; `#[async_trait]` keeps it `dyn`-compatible, which native `async fn`
/// in traits is not.
#[async_trait]
pub trait RunHandler: Send + Sync {
    async fn run(&self, claimed: ClaimedRun) -> HeadlessResult<()>;
}

#[derive(Debug, Clone)]
pub struct ClaimLoop {
    pub repo: HeadlessRunRepository,
    pub origin_kind: String,
    pub claimed_by: String,
    pub max_concurrent: usize,
    pub poll_every: Duration,
    pub renew_every: Duration,
    pub lease: Duration,
    pub stale_every: Duration,
    pub max_attempts: i32,
}

#[must_use]
pub fn claimer_id(hostname: &str) -> String {
    format!("{hostname}:{}", std::process::id())
}

impl ClaimLoop {
    #[must_use]
    pub fn new(repo: HeadlessRunRepository, origin_kind: &str, claimed_by: String) -> Self {
        Self {
            repo,
            origin_kind: origin_kind.to_owned(),
            claimed_by,
            max_concurrent: 2,
            poll_every: Duration::from_secs(2),
            renew_every: DEFAULT_RENEW_EVERY,
            lease: DEFAULT_LEASE,
            stale_every: Duration::from_secs(30),
            max_attempts: 2,
        }
    }

    pub async fn run(self, shutdown: CancellationToken, handler: Arc<dyn RunHandler>) {
        let mut in_flight: Vec<OwnedTask<()>> = Vec::new();
        let mut last_reclaim: Option<Instant> = None;
        loop {
            if last_reclaim.is_none_or(|at| at.elapsed() >= self.stale_every) {
                self.reclaim().await;
                last_reclaim = Some(Instant::now());
            }
            reap(&mut in_flight).await;
            while in_flight.len() < self.max_concurrent && !shutdown.is_cancelled() {
                match self
                    .repo
                    .claim_queued(&self.origin_kind, &self.claimed_by, self.lease)
                    .await
                {
                    Ok(Some(row)) => in_flight.push(self.spawn_run(row, &shutdown, &handler)),
                    Ok(None) => break,
                    Err(error) => {
                        tracing::error!(%error, origin_kind = %self.origin_kind, "headless claim failed");
                        break;
                    },
                }
            }
            tokio::select! {
                () = shutdown.cancelled() => break,
                () = tokio::time::sleep(self.poll_every) => {},
            }
        }
        for task in in_flight {
            log_join(task.join().await);
        }
    }

    async fn reclaim(&self) {
        match self
            .repo
            .reclaim_stale(&self.origin_kind, self.max_attempts)
            .await
        {
            Ok(ids) if !ids.is_empty() => {
                tracing::warn!(count = ids.len(), origin_kind = %self.origin_kind, "reclaimed stale headless runs");
            },
            Ok(_) => {},
            Err(error) => tracing::error!(%error, "headless stale reclaim failed"),
        }
    }

    fn spawn_run(
        &self,
        row: HeadlessRunRow,
        shutdown: &CancellationToken,
        handler: &Arc<dyn RunHandler>,
    ) -> OwnedTask<()> {
        let run_token = shutdown.child_token();
        let spec = LeaseSpec {
            run_id: row.id.clone(),
            claimed_by: self.claimed_by.clone(),
            every: self.renew_every,
            lease: self.lease,
        };
        let renewer = LeaseRenewer::start(self.repo.clone(), spec, run_token.clone());
        let claimed = ClaimedRun {
            row,
            run_token,
            repo: self.repo.clone(),
        };
        let handler = Arc::clone(handler);
        OwnedTask::spawn("headless_run", async move {
            let run_id = claimed.row.id.clone();
            if let Err(error) = handler.run(claimed).await {
                tracing::error!(%run_id, %error, "headless run handler failed");
            }
            renewer.stop().await;
        })
    }
}

async fn reap(in_flight: &mut Vec<OwnedTask<()>>) {
    let (done, running): (Vec<_>, Vec<_>) = std::mem::take(in_flight)
        .into_iter()
        .partition(OwnedTask::is_finished);
    *in_flight = running;
    for task in done {
        log_join(task.join().await);
    }
}

fn log_join(result: Result<(), tokio::task::JoinError>) {
    if let Err(error) = result {
        tracing::error!(%error, "headless run task panicked");
    }
}

/// Persists a run's events in `headless_run_events` and records the host
/// session id on the run row.
#[derive(Debug, Clone)]
pub struct DbEventSink {
    repo: HeadlessRunRepository,
    run_id: HeadlessRunId,
}

impl DbEventSink {
    #[must_use]
    pub const fn new(repo: HeadlessRunRepository, run_id: HeadlessRunId) -> Self {
        Self { repo, run_id }
    }
}

#[async_trait]
impl EventSink for DbEventSink {
    async fn emit(&self, batch: Vec<RunEvent>) -> HeadlessResult<()> {
        self.repo.insert_events_batch(&self.run_id, &batch).await
    }

    async fn session_observed(&self, session_id: &HostSessionId) -> HeadlessResult<()> {
        self.repo
            .update_host_session(&self.run_id, session_id)
            .await
    }
}
