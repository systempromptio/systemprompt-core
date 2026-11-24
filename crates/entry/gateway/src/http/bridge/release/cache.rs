//! A keyed cache that answers fresh within a TTL and keeps the last value
//! past it, so a failed refresh can still serve what was held.
//!
//! A miss is single-flight: [`Cache::flight`] hands out one lock per key, and
//! a caller that waited on it re-checks [`Cache::fresh_since`] before going
//! upstream, so concurrent misses on an expired entry cost one upstream call,
//! not one per waiting request. [`RefreshThrottle`] honours a forced refresh
//! per key at most once per interval; one asked for sooner is served from the
//! cache like any other read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

#[derive(Debug)]
pub(super) struct Cache<T> {
    held: RwLock<HashMap<String, (Instant, T)>>,
    flights: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            held: RwLock::new(HashMap::new()),
            flights: Mutex::new(HashMap::new()),
        }
    }
}

impl<T: Clone + Send + Sync> Cache<T> {
    pub(super) async fn fresh(&self, key: &str, ttl: Duration) -> Option<T> {
        self.fresh_since(key, ttl, None).await
    }

    pub(super) async fn fresh_since(
        &self,
        key: &str,
        ttl: Duration,
        since: Option<Instant>,
    ) -> Option<T> {
        let held = self.held.read().await;
        held.get(key)
            .filter(|(at, _)| since.map_or_else(|| at.elapsed() < ttl, |since| *at >= since))
            .map(|(_, value)| value.clone())
    }

    pub(super) async fn stale(&self, key: &str) -> Option<T> {
        let held = self.held.read().await;
        held.get(key).map(|(_, value)| value.clone())
    }

    pub(super) async fn store(&self, key: &str, value: T) {
        let mut held = self.held.write().await;
        held.insert(key.to_owned(), (Instant::now(), value));
    }

    pub(super) async fn flight(&self, key: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut flights = self.flights.lock().await;
            Arc::clone(flights.entry(key.to_owned()).or_default())
        };
        lock.lock_owned().await
    }
}

#[derive(Debug)]
pub(super) struct RefreshThrottle {
    interval: Duration,
    last: Mutex<HashMap<String, Instant>>,
}

impl RefreshThrottle {
    pub(super) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: Mutex::new(HashMap::new()),
        }
    }

    pub(super) async fn admit(&self, key: &str) -> bool {
        let mut last = self.last.lock().await;
        let admitted = last.get(key).is_none_or(|at| at.elapsed() >= self.interval);
        if admitted {
            last.insert(key.to_owned(), Instant::now());
        }
        admitted
    }
}
