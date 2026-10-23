//! One process-wide listener for new run events.
//!
//! Every event batch commits with a `pg_notify` on [`RUN_EVENTS_CHANNEL`]
//! carrying the run id. [`RunEventNotifier::spawn`] holds the single
//! `LISTEN` session for the process and fans each id out to in-memory
//! subscribers, so an open event stream costs no database session and wakes
//! on commit instead of polling. A notification can be lost (a reconnect,
//! a pooler that drops `LISTEN`), so subscribers also re-read on a slow
//! backstop interval; the listener only makes delivery prompt, never
//! correct.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use systemprompt_identifiers::AgentRunId;
use systemprompt_traits::BackgroundTasks;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub const RUN_EVENTS_CHANNEL: &str = "agent_run_events";
pub const BACKSTOP: Duration = Duration::from_secs(5);
const CAPACITY: usize = 1024;
const RECONNECT: Duration = Duration::from_secs(2);

/// Fan-out of committed run-event notifications to in-process subscribers.
#[derive(Debug, Clone)]
pub struct RunEventNotifier {
    sender: broadcast::Sender<AgentRunId>,
}

impl RunEventNotifier {
    #[must_use]
    pub fn spawn(tasks: &BackgroundTasks, pool: PgPool) -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        let notifier = Self { sender };
        let relay = notifier.clone();
        tasks.spawn_cancellable("agent_run_event_listener", move |cancel| {
            relay.listen(pool, cancel)
        });
        notifier
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AgentRunId> {
        self.sender.subscribe()
    }

    async fn listen(self, pool: PgPool, cancel: CancellationToken) {
        loop {
            match self.relay_until_error(&pool, &cancel).await {
                Ok(()) => return,
                Err(error) => {
                    tracing::warn!(%error, "agent run event listener lost; reconnecting");
                },
            }
            tokio::select! {
                () = cancel.cancelled() => return,
                () = tokio::time::sleep(RECONNECT) => {},
            }
        }
    }

    async fn relay_until_error(
        &self,
        pool: &PgPool,
        cancel: &CancellationToken,
    ) -> Result<(), sqlx::Error> {
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen(RUN_EVENTS_CHANNEL).await?;
        loop {
            let notification = tokio::select! {
                () = cancel.cancelled() => return Ok(()),
                received = listener.recv() => received?,
            };
            match AgentRunId::try_new(notification.payload()) {
                Ok(id) => {
                    if self.sender.send(id).is_err() {
                        tracing::trace!("no open event stream for this notification");
                    }
                },
                Err(error) => {
                    tracing::debug!(%error, "ignoring malformed run event notification");
                },
            }
        }
    }
}
