//! [`ClaimLoop`], the claim-and-dispatch loop, and [`DbEventSink`].
//!
//! An extension's `RunnerService` runs the loop; it claims queued runs with
//! `FOR UPDATE SKIP LOCKED` and hands each to a [`RunHandler`].
//! On shutdown the loop stops claiming, cancels in-flight runs' tokens so
//! their hosts stop, and waits for their handlers, which release each
//! interrupted run back to `queued` ([`ClaimedRun::interrupted`]) rather
//! than finishing it, up to `max_releases` times per run; a run interrupted
//! more often than that fails.
//! `claimer_id` formats the conventional `<hostname>:<pid>` claimer.
//! [`DbEventSink`] is the repository-backed [`EventSink`]; it writes under
//! the run's [`Lease`] and cancels the run once the lease is lost.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_identifiers::{AgentRunId, HostSessionId};
use systemprompt_traits::OwnedTask;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::error::{AgentRunsError, AgentRunsResult};
use crate::event::RunEvent;
use crate::execute::EventSink;
use crate::lease::{DEFAULT_LEASE, DEFAULT_RENEW_EVERY, LeaseRenewer, LeaseSpec};
use crate::repository::{AgentRunRepository, AgentRunRow, Claimed, Fenced, Lease};

/// A run the loop claimed and the [`Lease`] every write to it carries.
/// `run_token` is cancelled on shutdown, on a cancel request or when the
/// lease is lost; `shutdown` is the loop's own token.
#[derive(Debug, Clone)]
pub struct ClaimedRun {
    pub row: AgentRunRow,
    pub lease: Lease,
    pub run_token: CancellationToken,
    pub shutdown: CancellationToken,
    pub repo: AgentRunRepository,
    pub max_releases: i32,
}

impl ClaimedRun {
    #[must_use]
    pub fn new(
        claimed: Claimed,
        repo: AgentRunRepository,
        shutdown: &CancellationToken,
        max_releases: i32,
    ) -> Self {
        Self {
            max_releases,
            row: claimed.row,
            lease: claimed.lease,
            run_token: shutdown.child_token(),
            shutdown: shutdown.clone(),
            repo,
        }
    }

    #[must_use]
    pub fn interrupted(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}

/// Runs one claimed run to its end: calls `execute` and `finish`.
///
/// Held as `Arc<dyn RunHandler>` so the consumer picks the behaviour at
/// runtime; `#[async_trait]` keeps it `dyn`-compatible, which native `async fn`
/// in traits is not.
#[async_trait]
pub trait RunHandler: Send + Sync {
    async fn run(&self, claimed: ClaimedRun) -> AgentRunsResult<()>;
}

#[derive(Debug, Clone)]
pub struct ClaimLoop {
    pub repo: AgentRunRepository,
    pub origin_kind: String,
    pub claimed_by: String,
    pub max_concurrent: usize,
    pub poll_every: Duration,
    pub renew_every: Duration,
    pub lease: Duration,
    pub stale_every: Duration,
    pub max_attempts: i32,
    pub max_releases: i32,
}

#[must_use]
pub fn claimer_id(hostname: &str) -> String {
    format!("{hostname}:{}", std::process::id())
}

impl ClaimLoop {
    #[must_use]
    pub fn new(repo: AgentRunRepository, origin_kind: &str, claimed_by: String) -> Self {
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
            max_releases: 3,
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
                    Ok(Some(claimed)) => {
                        in_flight.push(self.spawn_run(claimed, &shutdown, &handler));
                    },
                    Ok(None) => break,
                    Err(error) => {
                        tracing::error!(%error, origin_kind = %self.origin_kind, "agent run claim failed");
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
                tracing::warn!(count = ids.len(), origin_kind = %self.origin_kind, "reclaimed stale agent runs");
            },
            Ok(_) => {},
            Err(error) => tracing::error!(%error, "agent run stale reclaim failed"),
        }
    }

    fn spawn_run(
        &self,
        claimed: Claimed,
        shutdown: &CancellationToken,
        handler: &Arc<dyn RunHandler>,
    ) -> OwnedTask<()> {
        let claimed = ClaimedRun::new(claimed, self.repo.clone(), shutdown, self.max_releases);
        let spec = LeaseSpec {
            run_id: claimed.row.id.clone(),
            lease: claimed.lease.clone(),
            every: self.renew_every,
            duration: self.lease,
        };
        let renewer = LeaseRenewer::start(self.repo.clone(), spec, claimed.run_token.clone());
        let handler = Arc::clone(handler);
        OwnedTask::spawn("agent_run", async move {
            let run_id = claimed.row.id.clone();
            if let Err(error) = handler.run(claimed).await {
                tracing::error!(%run_id, %error, "agent run handler failed");
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
        tracing::error!(%error, "agent run task panicked");
    }
}

/// Persists a claimed run's events in `agent_run_events` and records the
/// host session id on the run row, under the run's lease.
#[derive(Debug, Clone)]
pub struct DbEventSink {
    repo: AgentRunRepository,
    run_id: AgentRunId,
    lease: Lease,
    run_token: CancellationToken,
}

impl DbEventSink {
    #[must_use]
    pub fn new(claimed: &ClaimedRun) -> Self {
        Self {
            repo: claimed.repo.clone(),
            run_id: claimed.row.id.clone(),
            lease: claimed.lease.clone(),
            run_token: claimed.run_token.clone(),
        }
    }

    fn held(&self, fenced: Fenced) -> AgentRunsResult<()> {
        if fenced.is_applied() {
            return Ok(());
        }
        tracing::warn!(run_id = %self.run_id, "agent run lease lost; cancelling run");
        self.run_token.cancel();
        Err(AgentRunsError::LeaseLost {
            run_id: self.run_id.clone(),
        })
    }
}

#[async_trait]
impl EventSink for DbEventSink {
    async fn emit(&self, batch: Vec<RunEvent>) -> AgentRunsResult<()> {
        let fenced = self
            .repo
            .insert_events_batch(&self.run_id, &self.lease, &batch)
            .await?;
        self.held(fenced)
    }

    async fn session_observed(&self, session_id: &HostSessionId) -> AgentRunsResult<()> {
        let fenced = self
            .repo
            .update_host_session(&self.run_id, &self.lease, session_id)
            .await?;
        self.held(fenced)
    }
}
