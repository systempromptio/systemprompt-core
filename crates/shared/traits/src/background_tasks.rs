//! Owned background work: one tracker per process, drained at shutdown.
//!
//! [`BackgroundTasks`] is the single owner for work that outlives the call
//! that started it — audit completion after a response, analytics writes,
//! periodic loops. It is built once at the composition root, cloned into
//! whatever needs to spawn, and drained by the server's graceful shutdown, so
//! no task is detached: shutdown and tests observe the moment nothing is left
//! in flight.
//!
//! Two kinds of work run on it:
//!
//! - [`spawn`](BackgroundTasks::spawn) — one-shot work that runs to completion
//!   (a write, a settlement). Cancellation does not interrupt it; a drain waits
//!   for it within its timeout.
//! - [`spawn_cancellable`](BackgroundTasks::spawn_cancellable) — a loop or
//!   watcher handed a [`CancellationToken`] it must observe;
//!   [`shutdown`](BackgroundTasks::shutdown) cancels the token before it
//!   drains.
//!
//! Every task carries the tracing span and subscriber of the code that spawned
//! it, so its events correlate with the originating request and reach the same
//! collector. A task that panics is logged by name instead of vanishing.
//!
//! Work whose handle a single owner must join or abort (a listener, a relay,
//! a request-scoped worker) uses [`OwnedTask`], which stores its
//! [`JoinHandle`] and aborts it on drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use futures::FutureExt;
use tokio::task::{JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;

/// Process-wide owner of detached work; cloning shares the same tracker.
#[derive(Debug, Clone, Default)]
pub struct BackgroundTasks {
    tracker: TaskTracker,
    cancel: CancellationToken,
}

/// Result of waiting for in-flight background work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum DrainOutcome {
    Drained,
    TimedOut { in_flight: usize },
}

impl DrainOutcome {
    #[must_use]
    pub const fn is_drained(self) -> bool {
        matches!(self, Self::Drained)
    }
}

impl BackgroundTasks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn<F>(&self, name: &'static str, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tracker.spawn(
            supervised(name, task)
                .in_current_span()
                .with_current_subscriber(),
        );
    }

    pub fn spawn_cancellable<F, Fut>(&self, name: &'static str, task: F)
    where
        F: FnOnce(CancellationToken) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let token = self.cancel.child_token();
        self.spawn(name, task(token));
    }

    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.tracker.len()
    }

    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub async fn drain(&self, timeout: Duration) -> DrainOutcome {
        self.tracker.close();
        let waited = tokio::time::timeout(timeout, self.tracker.wait()).await;
        self.tracker.reopen();
        match waited {
            Ok(()) => DrainOutcome::Drained,
            Err(_elapsed) => DrainOutcome::TimedOut {
                in_flight: self.tracker.len(),
            },
        }
    }

    pub async fn shutdown(&self, timeout: Duration) -> DrainOutcome {
        self.cancel.cancel();
        self.drain(timeout).await
    }
}

async fn supervised<F>(name: &'static str, task: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if AssertUnwindSafe(task).catch_unwind().await.is_err() {
        tracing::error!(task = name, "Background task panicked");
    }
}

/// A single task whose owner joins or aborts it; dropping it aborts the task.
#[derive(Debug)]
#[must_use = "dropping an OwnedTask aborts it"]
pub struct OwnedTask<T> {
    name: &'static str,
    handle: JoinHandle<T>,
}

impl<T: Send + 'static> OwnedTask<T> {
    pub fn spawn<F>(name: &'static str, task: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        Self {
            name,
            handle: tokio::spawn(task.in_current_span().with_current_subscriber()),
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub fn abort(&self) {
        self.handle.abort();
    }

    pub async fn join(mut self) -> Result<T, JoinError> {
        (&mut self.handle).await
    }

    pub async fn abort_and_join(mut self) -> Option<T> {
        self.handle.abort();
        match (&mut self.handle).await {
            Ok(value) => Some(value),
            Err(e) if e.is_cancelled() => None,
            Err(e) => {
                tracing::error!(task = self.name, error = %e, "Owned task panicked");
                None
            },
        }
    }
}

impl<T> Drop for OwnedTask<T> {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
