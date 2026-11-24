//! The bounded queue every request-analytics write goes through.
//!
//! A tracked request enqueues its writes (session activity, velocity and
//! scanner checks, behavioural scoring, the analytics event) instead of
//! spawning a task per write. At most one writer task, owned by
//! [`BackgroundTasks`], runs at a time: it is started by the first write into
//! an idle queue, runs up to `writers` writes concurrently, and ends when the
//! queue is empty, so a drain still observes the moment nothing is pending.
//! A write offered to a full queue is dropped: it is counted on
//! [`HTTP_ANALYTICS_DROPPED_TOTAL`] and reported by a WARN at most once per
//! [`DROP_WARN_INTERVAL`]; the request is never delayed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_traits::BackgroundTasks;
use tokio::sync::mpsc;

pub const HTTP_ANALYTICS_DROPPED_TOTAL: &str = "http_analytics_dropped_total";
pub const DROP_WARN_INTERVAL: Duration = Duration::from_secs(10);

type Write = Pin<Box<dyn Future<Output = ()> + Send>>;

/// How an offered write was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offered {
    Queued,
    Dropped,
}

/// The process's request-analytics write queue; cloning shares it.
#[derive(Clone)]
pub struct AnalyticsQueue {
    inner: Arc<Inner>,
}

struct Inner {
    sender: mpsc::Sender<Write>,
    receiver: tokio::sync::Mutex<mpsc::Receiver<Write>>,
    writing: AtomicBool,
    writers: usize,
    background: BackgroundTasks,
    dropped: AtomicU64,
    last_warned: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for AnalyticsQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsQueue")
            .field("capacity", &self.inner.sender.max_capacity())
            .field("writers", &self.inner.writers)
            .field("dropped", &self.dropped())
            .finish_non_exhaustive()
    }
}

impl AnalyticsQueue {
    #[must_use]
    pub fn new(background: BackgroundTasks, capacity: NonZeroU32, writers: NonZeroU32) -> Self {
        let (sender, receiver) = mpsc::channel(capacity.get() as usize);
        Self {
            inner: Arc::new(Inner {
                sender,
                receiver: tokio::sync::Mutex::new(receiver),
                writing: AtomicBool::new(false),
                writers: writers.get() as usize,
                background,
                dropped: AtomicU64::new(0),
                last_warned: Mutex::new(None),
            }),
        }
    }

    pub fn offer<F>(&self, write: F) -> Offered
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if self.inner.sender.try_send(Box::pin(write)).is_err() {
            self.record_drop();
            return Offered::Dropped;
        }
        wake(&self.inner);
        Offered::Queued
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.inner.dropped.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.inner.sender.max_capacity() - self.inner.sender.capacity()
    }

    fn record_drop(&self) {
        let total = self.inner.dropped.fetch_add(1, Ordering::Relaxed) + 1;
        metrics::counter!(HTTP_ANALYTICS_DROPPED_TOTAL).increment(1);
        let mut last = self
            .inner
            .last_warned
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last.is_none_or(|at| at.elapsed() >= DROP_WARN_INTERVAL) {
            *last = Some(Instant::now());
            drop(last);
            tracing::warn!(
                dropped_total = total,
                capacity = self.inner.sender.max_capacity(),
                "Analytics queue is full; request analytics writes are being dropped"
            );
        }
    }
}

fn wake(inner: &Arc<Inner>) {
    if inner
        .writing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let inner = Arc::clone(inner);
        let background = inner.background.clone();
        background.spawn("analytics_writer", write_until_empty(inner));
    }
}

async fn write_until_empty(inner: Arc<Inner>) {
    loop {
        let batch = {
            let mut receiver = inner.receiver.lock().await;
            let mut batch = Vec::with_capacity(inner.writers);
            while batch.len() < inner.writers {
                match receiver.try_recv() {
                    Ok(write) => batch.push(write),
                    Err(_) => break,
                }
            }
            batch
        };
        if batch.is_empty() {
            inner.writing.store(false, Ordering::Release);
            let still_pending = inner.sender.capacity() < inner.sender.max_capacity();
            if still_pending
                && inner
                    .writing
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                continue;
            }
            return;
        }
        futures_util::future::join_all(batch).await;
    }
}
