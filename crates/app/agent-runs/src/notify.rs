//! One process-wide fan-out of new run events.
//!
//! Every event batch commits with a `pg_notify` on [`RUN_EVENTS_CHANNEL`]
//! carrying the run id. [`RunEventNotifier::spawn`] subscribes to that
//! channel on the node's [`NotificationHub`] and fans each id out to
//! in-memory subscribers, so an open event stream costs no database session
//! and wakes on commit instead of polling. A notification can be lost (a
//! reconnect, a pooler that drops `LISTEN`), so subscribers also re-read on
//! a slow backstop interval; the notification only makes delivery prompt,
//! never correct.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::{HubEvent, HubSubscription, NotificationHub};
use systemprompt_identifiers::AgentRunId;
use systemprompt_traits::BackgroundTasks;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub const RUN_EVENTS_CHANNEL: &str = "agent_run_events";
pub const BACKSTOP: Duration = Duration::from_secs(5);
const CAPACITY: usize = 1024;

/// Fan-out of committed run-event notifications to in-process subscribers.
#[derive(Debug, Clone)]
pub struct RunEventNotifier {
    sender: broadcast::Sender<AgentRunId>,
}

impl RunEventNotifier {
    #[must_use]
    pub fn spawn(tasks: &BackgroundTasks, hub: &NotificationHub) -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        let notifier = Self { sender };
        let relay = notifier.clone();
        let subscription = hub.subscribe(&[RUN_EVENTS_CHANNEL]);
        tasks.spawn_cancellable("agent_run_event_listener", move |cancel| {
            relay.relay(subscription, cancel)
        });
        notifier
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AgentRunId> {
        self.sender.subscribe()
    }

    async fn relay(self, mut subscription: HubSubscription, cancel: CancellationToken) {
        loop {
            let event = tokio::select! {
                () = cancel.cancelled() => return,
                event = subscription.recv() => event,
            };
            let HubEvent::Notification(notification) = event else {
                continue;
            };
            match AgentRunId::try_new(notification.payload.as_ref()) {
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
