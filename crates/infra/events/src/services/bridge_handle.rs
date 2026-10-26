//! The relay task's handle: tri-state status for `/health` and the
//! cancellation that stops it cleanly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_traits::OwnedTask;

use super::bridge::PostgresEventBridge;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Where the relay task stands, as reported by `/health`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStatus {
    NotStarted,
    Listening,
    Reconnecting,
    Stopped,
}

impl RelayStatus {
    #[must_use]
    pub const fn is_listening(self) -> bool {
        matches!(self, Self::Listening)
    }
}

#[derive(Debug)]
pub(super) struct StatusCell(watch::Sender<RelayStatus>);

impl Default for StatusCell {
    fn default() -> Self {
        Self(watch::Sender::new(RelayStatus::NotStarted))
    }
}

impl StatusCell {
    pub(super) fn set(&self, status: RelayStatus) {
        self.0.send_replace(status);
    }

    pub(super) fn get(&self) -> RelayStatus {
        *self.0.borrow()
    }

    async fn listening(&self) -> bool {
        let mut status = self.0.subscribe();
        status
            .wait_for(|s| matches!(s, RelayStatus::Listening | RelayStatus::Stopped))
            .await
            .is_ok_and(|s| s.is_listening())
    }
}

/// The relay: built idle, started once, then its task, its status and the
/// token that stops it.
///
/// [`listening`](Self::listening) resolves once the relay's `LISTEN` is in
/// place, so a notification committed afterwards is guaranteed to reach it,
/// or with `false` once the relay has stopped. Dropping the handle aborts the
/// task; call [`EventBridgeHandle::shutdown`] so the listener session closes
/// cleanly before the pool does. `start` and `shutdown` take `&self` so the
/// handle can live in a shared context; a second call of either is a no-op.
#[derive(Debug)]
pub struct EventBridgeHandle {
    pending: Mutex<Option<PostgresEventBridge>>,
    task: Mutex<Option<OwnedTask<()>>>,
    status: Arc<StatusCell>,
    cancel: CancellationToken,
}

impl EventBridgeHandle {
    pub(super) fn idle(bridge: PostgresEventBridge) -> Self {
        Self {
            pending: Mutex::new(Some(bridge)),
            task: Mutex::new(None),
            status: Arc::new(StatusCell::default()),
            cancel: CancellationToken::new(),
        }
    }

    pub fn start(&self) -> bool {
        let Some(bridge) = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        else {
            return false;
        };
        let task = OwnedTask::spawn("event_bridge", {
            let status = Arc::clone(&self.status);
            let cancel = self.cancel.clone();
            async move {
                bridge.run(&status, cancel).await;
            }
        });
        *self.task.lock().unwrap_or_else(PoisonError::into_inner) = Some(task);
        true
    }

    #[must_use]
    pub fn status(&self) -> RelayStatus {
        self.status.get()
    }

    pub async fn listening(&self) -> bool {
        self.status.listening().await
    }

    pub async fn shutdown(&self) {
        self.cancel.cancel();
        let task = self
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(task) = task
            && let Err(error) = task.join().await
        {
            warn!(error = %error, "event bridge task did not exit cleanly");
        }
    }
}
