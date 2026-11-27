//! Owned background work: one tracker per process, drained at shutdown.
//!
//! [`BackgroundTasks`] is the single owner for work that outlives the call
//! that started it — audit completion after a response, analytics writes,
//! periodic loops. It is built once at the composition root, cloned into
//! whatever needs to spawn, and drained by the server's graceful shutdown, so
//! no task is detached: shutdown and tests observe the moment nothing is left
//! in flight.
//!
//! Three kinds of work run on it:
//!
//! - [`spawn`](BackgroundTasks::spawn) — one-shot work that runs to completion
//!   (a write, a settlement). Cancellation does not interrupt it; a drain waits
//!   for it within its timeout.
//! - [`spawn_cancellable`](BackgroundTasks::spawn_cancellable) — a loop or
//!   watcher handed a [`CancellationToken`] it must observe;
//!   [`shutdown`](BackgroundTasks::shutdown) cancels the token before it
//!   drains.
//! - [`spawn_blocking`](BackgroundTasks::spawn_blocking) — synchronous work on
//!   the blocking pool; it is drained but can never be interrupted.
//!
//! [`shutdown_or_abort`](BackgroundTasks::shutdown_or_abort) is the shutdown
//! for an owner that must not leave work behind: whatever outlives the timeout
//! is dropped where it is suspended and logged by name. A set that has aborted
//! runs nothing spawned on it afterwards.
//!
//! Every task carries the tracing span and subscriber of the code that spawned
//! it, so its events correlate with the originating request and reach the same
//! collector. A task that panics is logged by name instead of vanishing.
//!
//! It also carries the process's [`crate::StreamDrain`], so anything that
//! can spawn can also see when long-lived response streams must end.
//!
//! Work whose handle a single owner must join or abort (a listener, a relay,
//! a request-scoped worker) uses [`OwnedTask`], which stores its
//! [`JoinHandle`] and aborts it on drop; a join that must name the task it
//! lost fails with [`TaskError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use futures::FutureExt;
use tokio::task::{JoinError, JoinHandle};
pub use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;

/// Process-wide owner of detached work; cloning shares the same tracker.
#[derive(Debug, Clone, Default)]
pub struct BackgroundTasks {
    tracker: TaskTracker,
    cancel: CancellationToken,
    abort: CancellationToken,
    streams: crate::StreamDrain,
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
            supervised(name, self.abort.clone(), task)
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

    pub fn spawn_blocking<F>(&self, name: &'static str, work: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let span = tracing::Span::current();
        self.tracker.spawn_blocking(move || {
            span.in_scope(|| {
                if std::panic::catch_unwind(AssertUnwindSafe(work)).is_err() {
                    tracing::error!(task = name, "Background task panicked");
                }
            });
        });
    }

    #[must_use]
    pub const fn stream_drain(&self) -> &crate::StreamDrain {
        &self.streams
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

    pub async fn shutdown_or_abort(&self, timeout: Duration) -> DrainOutcome {
        let outcome = self.shutdown(timeout).await;
        if let DrainOutcome::TimedOut { in_flight } = outcome {
            tracing::warn!(
                in_flight,
                "Background tasks outlived shutdown; aborting them"
            );
            self.abort.cancel();
        }
        outcome
    }
}

async fn supervised<F>(name: &'static str, abort: CancellationToken, task: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::select! {
        biased;
        outcome = AssertUnwindSafe(task).catch_unwind() => {
            if outcome.is_err() {
                tracing::error!(task = name, "Background task panicked");
            }
        },
        () = abort.cancelled() => {
            tracing::warn!(task = name, "Background task aborted at shutdown");
        },
    }
}

/// Why an [`OwnedTask`] did not hand back its value, naming the task.
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("task {name} was cancelled")]
    Cancelled {
        name: &'static str,
        #[source]
        source: JoinError,
    },
    #[error("task {name} panicked")]
    Panicked {
        name: &'static str,
        #[source]
        source: JoinError,
    },
}

impl TaskError {
    #[must_use]
    pub fn from_join(name: &'static str, source: JoinError) -> Self {
        if source.is_cancelled() {
            Self::Cancelled { name, source }
        } else {
            Self::Panicked { name, source }
        }
    }

    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }

    #[must_use]
    pub const fn task_name(&self) -> &'static str {
        match self {
            Self::Cancelled { name, .. } | Self::Panicked { name, .. } => name,
        }
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

    pub fn spawn_blocking<F>(name: &'static str, work: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let span = tracing::Span::current();
        Self {
            name,
            handle: tokio::task::spawn_blocking(move || span.in_scope(work)),
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

    pub async fn join_named(self) -> Result<T, TaskError> {
        let name = self.name;
        self.join()
            .await
            .map_err(|source| TaskError::from_join(name, source))
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
