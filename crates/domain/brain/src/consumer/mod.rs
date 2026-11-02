//! [`BrainConsumer`]: the runner service draining brain events from the
//! durable outbox through the [`BrainPipeline`].
//!
//! Each round claims up to one profile batch of events on one transaction.
//! Each event is processed inside its own savepoint and acknowledged on the
//! round's transaction, which commits once, so one bad event rolls back
//! only itself and a crash rolls back the round in hand. The round shares
//! one [`BatchContext`](crate::pipeline::BatchContext).
//!
//! An event that cannot succeed (it does not decode, its version is one
//! this build does not read, or a stage refuses it for good) is recorded
//! as a failure and acknowledged. An event that fails for a reason that may
//! pass (storage, a seam call, a timeout, a spent token budget) is rolled
//! back to its savepoint, recorded, left unacknowledged and held back for
//! the poll interval while the round goes on; its [`MAX_EVENT_ATTEMPTS`]th
//! failure is recorded as abandoned and acknowledged. More than
//! [`MAX_ROUND_TRANSIENTS`] transient failures end the round early.
//!
//! When nothing is pending it listens on the durable facts channel with the
//! poll interval as a backstop, and every `sweep_secs` it runs
//! [`BrainPipeline::sweep`] under the brain's own advisory lock. The runtime
//! starts it as a runner service; it spawns nothing itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builder;
mod drain;
mod handle;
mod round_state;
mod wake;

pub use builder::BrainConsumerBuilder;
pub use drain::MAX_ROUND_TRANSIENTS;
pub use handle::MAX_EVENT_ATTEMPTS;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_manifest::profile::BrainConsumerProfile;
use systemprompt_models::brain::BRAIN_CONSUMER;
use systemprompt_traits::{JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use self::round_state::RoundState;
use self::wake::Wake;
use crate::error::BrainResult;
use crate::pipeline::BrainPipeline;

/// What the consumer is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerState {
    Idle,
    Draining,
    Listening,
    Reconnecting,
}

impl ConsumerState {
    const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Draining,
            2 => Self::Listening,
            3 => Self::Reconnecting,
            _ => Self::Idle,
        }
    }
}

/// The consumer's state, its event counts since start and when its last
/// round ran, as health reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BacklogSnapshot {
    pub processed: u64,
    pub retried: u64,
    pub failed: u64,
    pub abandoned: u64,
    pub state: ConsumerState,
    pub last_round: Option<DateTime<Utc>>,
}

/// Events waiting in the outbox for the brain consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PendingEvents {
    pub pending: i64,
    pub oldest_seconds: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StateCell(Arc<AtomicU8>);

impl StateCell {
    pub(crate) fn set(&self, state: ConsumerState) {
        self.0.store(state as u8, Ordering::Relaxed);
    }

    fn get(&self) -> ConsumerState {
        ConsumerState::from_u8(self.0.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Default)]
pub(crate) struct Counters {
    pub(crate) processed: AtomicU64,
    pub(crate) retried: AtomicU64,
    pub(crate) failed: AtomicU64,
    pub(crate) abandoned: AtomicU64,
}

impl Counters {
    pub(crate) fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Drains brain events from the outbox through the pipeline.
pub struct BrainConsumer {
    pipeline: Arc<BrainPipeline>,
    outbox: Arc<OutboxConsumer>,
    pool: PgPool,
    listener_pool: PgPool,
    settings: BrainConsumerProfile,
    state: StateCell,
    counters: Arc<Counters>,
    round_state: RoundState,
}

impl std::fmt::Debug for BrainConsumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainConsumer")
            .field("settings", &self.settings)
            .field("state", &self.state.get())
            .finish_non_exhaustive()
    }
}

impl BrainConsumer {
    #[must_use]
    pub const fn pipeline(&self) -> &Arc<BrainPipeline> {
        &self.pipeline
    }

    #[must_use]
    pub fn backlog(&self) -> BacklogSnapshot {
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        BacklogSnapshot {
            processed: read(&self.counters.processed),
            retried: read(&self.counters.retried),
            failed: read(&self.counters.failed),
            abandoned: read(&self.counters.abandoned),
            state: self.state.get(),
            last_round: self.round_state.last_round(),
        }
    }

    /// The outbox rows waiting for this consumer, held-back retries
    /// included.
    pub async fn pending(&self) -> BrainResult<PendingEvents> {
        let (pending, oldest_seconds) = self.outbox.pending(BRAIN_CONSUMER).await?;
        Ok(PendingEvents {
            pending,
            oldest_seconds,
        })
    }

    fn batch(&self) -> usize {
        usize::try_from(self.settings.batch.max(1)).unwrap_or(usize::MAX)
    }

    async fn sweep(&self) {
        match self.pipeline.sweep(Utc::now()).await {
            Ok(Some(report)) => tracing::debug!(
                facts = report.facts.total(),
                open_disputes = report.open_disputes,
                tokens_today = report.tokens_today,
                "Brain sweep reported"
            ),
            Ok(None) => tracing::debug!("Brain sweep held by another replica"),
            Err(error) => tracing::error!(error = %error, "Brain sweep failed"),
        }
    }
}

#[async_trait]
impl RunnerService for BrainConsumer {
    fn name(&self) -> &'static str {
        "brain-consumer"
    }

    async fn run(&self, _ctx: JobContext, shutdown: CancellationToken) {
        let poll = Duration::from_secs(u64::from(self.settings.poll_secs.max(1)));
        let sweep_every = Duration::from_secs(u64::from(self.settings.sweep_secs.max(1)));
        let mut wake = Wake::new(self.listener_pool.clone(), self.state.clone());
        let mut next_sweep = Instant::now();
        while !shutdown.is_cancelled() {
            self.state.set(ConsumerState::Draining);
            let worked = match self.drain_round().await {
                Ok(worked) => worked,
                Err(error) => {
                    tracing::error!(error = %error, "Brain drain round failed");
                    false
                },
            };
            if Instant::now() >= next_sweep {
                self.sweep().await;
                next_sweep = Instant::now() + sweep_every;
            }
            if !worked {
                let until_sweep = next_sweep.saturating_duration_since(Instant::now());
                if !wake.wait(poll.min(until_sweep), &shutdown).await {
                    break;
                }
            }
        }
        self.state.set(ConsumerState::Idle);
    }
}
