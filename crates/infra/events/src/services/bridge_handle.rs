//! The relay task's handle: tri-state status for `/health` and the
//! cancellation that stops it cleanly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinHandle;
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

/// The running relay: its task, its status and the token that stops it.
///
/// [`listening`](Self::listening) resolves once the relay's `LISTEN` is in
/// place, so a notification committed afterwards is guaranteed to reach it,
/// or with `false` once the relay has stopped. Dropping the handle does not
/// stop the task; call
/// [`EventBridgeHandle::shutdown`] so the listener session closes before
/// the pool does. `shutdown` takes `&self` so the handle can live in a
/// shared `OnceLock`; a second call is a no-op.
#[derive(Debug)]
pub struct EventBridgeHandle {
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    status: Arc<StatusCell>,
    cancel: CancellationToken,
}

impl EventBridgeHandle {
    pub(super) fn new(
        task: JoinHandle<()>,
        status: Arc<StatusCell>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            task: tokio::sync::Mutex::new(Some(task)),
            status,
            cancel,
        }
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
        let task = self.task.lock().await.take();
        if let Some(task) = task
            && let Err(error) = task.await
        {
            warn!(error = %error, "event bridge task did not exit cleanly");
        }
    }
}
