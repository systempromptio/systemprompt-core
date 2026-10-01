//! Owned per-request background work.
//!
//! Audit completion, settlement, accounting and post-response scans finish
//! after the response has been handed to the caller. They run on
//! [`GatewayBackgroundTasks`], one tracker shared by every request served
//! through a [`GatewayRepositories`](super::GatewayRepositories), so none of
//! that work is detached: a shutdown or a test can
//! [`drain`](GatewayBackgroundTasks::drain) it and observe the moment nothing
//! is left in flight. Draining does not refuse new work; it resolves once the
//! tracker is empty. Each task carries the span and subscriber of the request
//! that spawned it, so its events correlate with that request and reach the
//! same collector.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use tokio_util::task::TaskTracker;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;

/// Tracker for the gateway work that outlives a response.
#[derive(Debug, Clone, Default)]
pub struct GatewayBackgroundTasks {
    tracker: TaskTracker,
}

impl GatewayBackgroundTasks {
    pub fn spawn<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tracker
            .spawn(task.in_current_span().with_current_subscriber());
    }

    pub async fn drain(&self) {
        self.tracker.close();
        self.tracker.wait().await;
    }
}
