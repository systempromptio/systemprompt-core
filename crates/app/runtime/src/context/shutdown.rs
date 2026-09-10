//! In-process request for a graceful restart.
//!
//! An admin action that swaps the services root cannot take effect in the
//! running process: the loader, the router and every cached catalog were
//! built from the old tree. Rather than rebuild them in place, the handler
//! asks the process to drain and exit, and the supervisor starts it again on
//! the new composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use tokio::sync::Notify;

/// Handle used to ask the running process to drain and exit.
///
/// One handle is created at the composition root and shared by clone between
/// the [`super::AppContext`] and the server's shutdown selector; the server
/// binds its listener before the context exists, so both must be handed the
/// same handle rather than each making its own. A request raised before
/// anyone is waiting is held as a permit and delivered to the first waiter,
/// so a restart asked for during boot is not lost.
#[derive(Debug, Clone)]
pub struct ShutdownRequest {
    notify: Arc<Notify>,
}

impl Default for ShutdownRequest {
    fn default() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
        }
    }
}

impl ShutdownRequest {
    pub fn request(&self, reason: &str) {
        tracing::warn!(reason = %reason, "Graceful restart requested");
        self.notify.notify_one();
    }

    pub async fn requested(&self) {
        self.notify.notified().await;
    }
}
