//! The relay task's handle: tri-state status for `/health` and the
//! cancellation that stops it cleanly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

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
    const fn as_u8(self) -> u8 {
        match self {
            Self::NotStarted => 0,
            Self::Listening => 1,
            Self::Reconnecting => 2,
            Self::Stopped => 3,
        }
    }

    const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Listening,
            2 => Self::Reconnecting,
            3 => Self::Stopped,
            _ => Self::NotStarted,
        }
    }

    #[must_use]
    pub const fn is_listening(self) -> bool {
        matches!(self, Self::Listening)
    }
}

#[derive(Debug, Default)]
pub(super) struct StatusCell(AtomicU8);

impl StatusCell {
    pub(super) fn set(&self, status: RelayStatus) {
        self.0.store(status.as_u8(), Ordering::Release);
    }

    pub(super) fn get(&self) -> RelayStatus {
        RelayStatus::from_u8(self.0.load(Ordering::Acquire))
    }
}

/// The running relay: its task, its status and the token that stops it.
///
/// Dropping the handle does not stop the task; call
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
