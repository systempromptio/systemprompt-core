//! [`ConfigWatch`]: one parsed config plane held in memory and replaced
//! whenever any node changes it.
//!
//! Readers call [`ConfigWatch::get`], an atomic pointer load that never
//! blocks. A driver task ([`ConfigWatch::spawn`]) reloads the plane through
//! the caller's async `reload` when the plane's [`CacheDomain`] epoch advances
//! (the `config:<plane>` notification), and polls a [`ConfigVersionSource`]
//! every `poll` interval as the fallback for a lost notification: a moved
//! `config_version` reloads too. A failed reload keeps serving the last good
//! value and is retried on the next signal or poll.
//!
//! The version is read before the reload, so a change committed while a
//! reload runs moves the version again and the next poll picks it up.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::{BackgroundTasks, CacheDomain, CacheEpochs, RepositoryError};

pub const CONFIG_WATCH_POLL: Duration = Duration::from_secs(30);

/// Where the watch reads the cluster-wide config version from.
///
/// Held as `Arc<dyn ConfigVersionSource>` so the database implementation and
/// a test fake are interchangeable, which is why it uses `async_trait`.
#[async_trait]
pub trait ConfigVersionSource: Send + Sync {
    async fn current_version(&self) -> Result<i64, RepositoryError>;
}

pub struct ConfigWatch<T> {
    current: Arc<ArcSwap<T>>,
    reloads: Arc<AtomicU64>,
}

impl<T> Clone for ConfigWatch<T> {
    fn clone(&self) -> Self {
        Self {
            current: Arc::clone(&self.current),
            reloads: Arc::clone(&self.reloads),
        }
    }
}

impl<T> fmt::Debug for ConfigWatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigWatch")
            .field("reloads", &self.reloads.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct WatchSignals {
    pub domain: CacheDomain,
    pub epochs: CacheEpochs,
    pub version: Arc<dyn ConfigVersionSource>,
    pub poll: Duration,
}

impl fmt::Debug for dyn ConfigVersionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConfigVersionSource")
    }
}

impl<T: Send + Sync + 'static> ConfigWatch<T> {
    #[must_use]
    pub fn new(initial: T) -> Self {
        Self {
            current: Arc::new(ArcSwap::from_pointee(initial)),
            reloads: Arc::new(AtomicU64::new(0)),
        }
    }

    #[must_use]
    pub fn get(&self) -> Arc<T> {
        self.current.load_full()
    }

    #[must_use]
    pub fn reloads(&self) -> u64 {
        self.reloads.load(Ordering::Acquire)
    }

    pub fn spawn<F, Fut, E>(&self, tasks: &BackgroundTasks, signals: WatchSignals, reload: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        E: fmt::Display + Send + 'static,
    {
        let watch = self.clone();
        tasks.spawn_cancellable("config_watch", move |cancel| {
            watch.run(signals, reload, cancel)
        });
    }

    pub async fn run<F, Fut, E>(self, signals: WatchSignals, reload: F, cancel: CancellationToken)
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, E>> + Send,
        E: fmt::Display,
    {
        let mut epoch = signals.epochs.subscribe(signals.domain);
        let mut seen = read_version(&signals).await;
        let mut ticker = tokio::time::interval(signals.poll);
        ticker.reset();
        loop {
            let polled = tokio::select! {
                () = cancel.cancelled() => return,
                changed = epoch.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    false
                },
                _ = ticker.tick() => true,
            };
            let version = read_version(&signals).await;
            if polled && (version.is_none() || version == seen) {
                continue;
            }
            if self.reload_once(&reload, signals.domain).await {
                seen = version;
            }
        }
    }

    async fn reload_once<F, Fut, E>(&self, reload: &F, domain: CacheDomain) -> bool
    where
        F: Fn() -> Fut + Sync,
        Fut: Future<Output = Result<T, E>> + Send,
        E: fmt::Display,
    {
        match reload().await {
            Ok(value) => {
                self.current.store(Arc::new(value));
                self.reloads.fetch_add(1, Ordering::AcqRel);
                true
            },
            Err(error) => {
                warn!(domain = domain.label(), %error, "config watch reload failed; keeping the last value");
                false
            },
        }
    }
}

async fn read_version(signals: &WatchSignals) -> Option<i64> {
    match signals.version.current_version().await {
        Ok(version) => Some(version),
        Err(error) => {
            warn!(domain = signals.domain.label(), %error, "config version poll failed");
            None
        },
    }
}
