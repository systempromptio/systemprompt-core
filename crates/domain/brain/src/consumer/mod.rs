//! [`BrainConsumer`]: the runner service draining brain events from the
//! durable outbox through the [`BrainPipeline`].
//!
//! Each round claims up to one profile batch of events, one at a time,
//! each on its own transaction: the event is processed inside a savepoint
//! and the outbox row is acknowledged in the same transaction, so one bad
//! event never holds the others and a crash rolls back only the event in
//! hand. The round shares one
//! [`BatchContext`](crate::pipeline::BatchContext).
//!
//! An event that cannot succeed (it does not decode, its version is one
//! this build does not read, or a stage refuses it for good) is recorded
//! as a failure and acknowledged. An event that fails for a reason that may
//! pass (storage, a seam call, a timeout, a spent token budget) is rolled
//! back unacknowledged, recorded, and retried from the next round after the
//! poll interval; its [`MAX_EVENT_ATTEMPTS`]th failure is recorded as
//! abandoned and acknowledged.
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
mod wake;

pub use builder::BrainConsumerBuilder;
pub use drain::MAX_EVENT_ATTEMPTS;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_manifest::profile::BrainConsumerProfile;
use systemprompt_traits::{JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use self::wake::Wake;
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

/// The consumer's state and its event counts since start, as health
/// reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BacklogSnapshot {
    pub processed: u64,
    pub retried: u64,
    pub failed: u64,
    pub abandoned: u64,
    pub state: ConsumerState,
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
        }
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
