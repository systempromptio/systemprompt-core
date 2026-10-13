//! Lease renewal for a claimed run.
//!
//! A [`LeaseRenewer`] extends the run's lease on a fixed cadence. When the
//! lease is lost (another claimer reclaimed the run, or it turned terminal)
//! or renewal keeps failing until the lease would have lapsed, it cancels
//! the run's token so the executor stops the host. Stale reclaim itself is
//! [`AgentRunRepository::reclaim_stale`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::AgentRunId;
use systemprompt_traits::OwnedTask;
use tokio::time::{Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use crate::repository::AgentRunRepository;

pub const DEFAULT_RENEW_EVERY: Duration = Duration::from_secs(30);
pub const DEFAULT_LEASE: Duration = Duration::from_secs(90);

/// Owns the renewal task; dropping it stops renewal.
#[derive(Debug)]
pub struct LeaseRenewer {
    task: OwnedTask<()>,
}

/// What a renewer renews and how often.
#[derive(Debug, Clone)]
pub struct LeaseSpec {
    pub run_id: AgentRunId,
    pub claimed_by: String,
    pub every: Duration,
    pub lease: Duration,
}

impl LeaseRenewer {
    pub fn start(repo: AgentRunRepository, spec: LeaseSpec, token: CancellationToken) -> Self {
        Self {
            task: OwnedTask::spawn("agent_run_lease_renewer", renew_loop(repo, spec, token)),
        }
    }

    pub async fn stop(self) {
        self.task.abort_and_join().await;
    }
}

async fn renew_loop(repo: AgentRunRepository, spec: LeaseSpec, token: CancellationToken) {
    let mut ticker = tokio::time::interval_at(Instant::now() + spec.every, spec.every);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_renewed = Instant::now();
    loop {
        tokio::select! {
            () = token.cancelled() => return,
            _ = ticker.tick() => {},
        }
        match repo
            .renew_lease(&spec.run_id, &spec.claimed_by, spec.lease)
            .await
        {
            Ok(true) => last_renewed = Instant::now(),
            Ok(false) => {
                tracing::warn!(run_id = %spec.run_id, "agent run lease lost; cancelling run");
                token.cancel();
                return;
            },
            Err(error) if last_renewed.elapsed() >= spec.lease => {
                tracing::error!(run_id = %spec.run_id, %error, "agent run lease lapsed; cancelling run");
                token.cancel();
                return;
            },
            Err(error) => {
                tracing::warn!(run_id = %spec.run_id, %error, "agent run lease renewal failed; retrying");
            },
        }
    }
}
