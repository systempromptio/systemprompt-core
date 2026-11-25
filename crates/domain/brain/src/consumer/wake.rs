//! The consumer's wake-up: a subscription to the shared facts channel on
//! the node's notification hub, with the poll interval as the delivery
//! guarantee. While the hub is not listening the consumer polls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::{NotificationHub, PollWake, WakeReason};
use systemprompt_events::NOTIFICATION_FACTS_CHANNEL;
use tokio_util::sync::CancellationToken;

use super::{ConsumerState, StateCell};

pub(super) struct Wake {
    wake: PollWake,
    state: StateCell,
}

impl Wake {
    pub(super) fn new(hub: &NotificationHub, state: StateCell) -> Self {
        Self {
            wake: PollWake::new(hub, &[NOTIFICATION_FACTS_CHANNEL]),
            state,
        }
    }

    pub(super) async fn wait(&mut self, poll: Duration, shutdown: &CancellationToken) -> bool {
        self.state.set(if self.wake.is_listening() {
            ConsumerState::Listening
        } else {
            ConsumerState::Reconnecting
        });
        self.wake.wait(poll, shutdown).await != WakeReason::Shutdown
    }

    pub(super) async fn back_off(&self, wait: Duration, shutdown: &CancellationToken) -> bool {
        tokio::select! {
            () = shutdown.cancelled() => false,
            () = tokio::time::sleep(wait) => true,
        }
    }
}
