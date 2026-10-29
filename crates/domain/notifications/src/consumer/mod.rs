//! [`NotificationsConsumer`]: the runner service draining notification facts
//! from the durable outbox.
//!
//! Each round claims up to one profile batch of facts, one at a time, each
//! on its own transaction: the fact is processed inside a savepoint and the
//! outbox row is acknowledged in the same transaction, so one large fact
//! never holds the others and a crash rolls back only the fact in hand.
//!
//! A fact that cannot succeed (it does not decode, or its version is one this
//! build does not read) is recorded as a `decode` failure and acknowledged.
//! A fact that fails for a reason that may pass (storage, a seam call, a
//! router timeout) is rolled back unacknowledged, recorded as a `process`
//! failure and retried from the next round after the poll interval; its
//! [`MAX_FACT_ATTEMPTS`]th failure is recorded as `fact_abandoned` and
//! acknowledged, so one bad fact never blocks the queue. Rows a committed
//! fact left `pending` are delivered right after the commit. The round shares
//! one [`BatchContext`](crate::pipeline::BatchContext).
//!
//! When nothing is pending it listens on
//! [`NOTIFICATION_FACTS_CHANNEL`](systemprompt_events::NOTIFICATION_FACTS_CHANNEL)
//! on the listener pool, with the poll interval as a backstop because a
//! notification can be lost on reconnect; a listener error reconnects with
//! backoff and never ends the runner. Every `sweep_secs` the consumer runs
//! [`NotificationPipeline::sweep`], which holds a transaction-scoped
//! advisory lock, so one replica sweeps at a time.
//!
//! The runtime builder constructs and starts it, because it owns the seams
//! the pipeline is assembled from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod drain;
mod wake;

pub use drain::MAX_FACT_ATTEMPTS;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_manifest::profile::ConsumerProfile;
use systemprompt_traits::{JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use self::wake::Wake;
use crate::error::NotificationsResult;
use crate::pipeline::{NotificationPipeline, metrics};

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

/// The delivery backlog and the consumer's state, as health reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BacklogSnapshot {
    pub pending: i64,
    pub oldest_seconds: Option<i64>,
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

/// Drains notification facts from the outbox through the pipeline.
pub struct NotificationsConsumer {
    pipeline: Arc<NotificationPipeline>,
    outbox: OutboxConsumer,
    pool: PgPool,
    listener_pool: PgPool,
    settings: ConsumerProfile,
    state: StateCell,
}

impl std::fmt::Debug for NotificationsConsumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationsConsumer")
            .field("settings", &self.settings)
            .field("state", &self.state.get())
            .finish_non_exhaustive()
    }
}

impl NotificationsConsumer {
    #[must_use]
    pub fn new(
        pipeline: Arc<NotificationPipeline>,
        pool: PgPool,
        listener_pool: PgPool,
        settings: ConsumerProfile,
    ) -> Self {
        Self {
            pipeline,
            outbox: OutboxConsumer::new(pool.clone()),
            pool,
            listener_pool,
            settings,
            state: StateCell::default(),
        }
    }

    pub async fn backlog(&self) -> NotificationsResult<BacklogSnapshot> {
        let (pending, oldest_seconds) =
            self.pipeline.repositories().notifications.backlog().await?;
        metrics::backlog(pending, oldest_seconds);
        Ok(BacklogSnapshot {
            pending,
            oldest_seconds,
            state: self.state.get(),
        })
    }

    fn batch(&self) -> usize {
        usize::try_from(self.settings.batch.max(1)).unwrap_or(usize::MAX)
    }

    async fn sweep(&self) {
        match self.pipeline.sweep(Utc::now()).await {
            Ok(outcome) if outcome.delivered + outcome.failed > 0 => tracing::debug!(
                delivered = outcome.delivered,
                failed = outcome.failed,
                "Notification sweep delivered"
            ),
            Ok(_) => {},
            Err(error) => tracing::error!(error = %error, "Notification sweep failed"),
        }
    }

    async fn report_backlog(&self) {
        if let Err(error) = self.backlog().await {
            tracing::warn!(error = %error, "Notification backlog could not be read");
        }
    }
}

#[async_trait]
impl RunnerService for NotificationsConsumer {
    fn name(&self) -> &'static str {
        "notifications-consumer"
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
                    tracing::error!(error = %error, "Notification drain round failed");
                    false
                },
            };
            if Instant::now() >= next_sweep {
                self.sweep().await;
                self.report_backlog().await;
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
