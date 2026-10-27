//! A keyed cache that answers fresh within a TTL and keeps the last value
//! past it, so a failed refresh can still serve what was held.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(super) struct Cache<T>(tokio::sync::RwLock<HashMap<String, (Instant, T)>>);

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self(tokio::sync::RwLock::new(HashMap::new()))
    }
}

impl<T: Clone + Send + Sync> Cache<T> {
    pub(super) async fn fresh(&self, key: &str, ttl: Duration) -> Option<T> {
        let held = self.0.read().await;
        held.get(key)
            .filter(|(at, _)| at.elapsed() < ttl)
            .map(|(_, value)| value.clone())
    }

    pub(super) async fn stale(&self, key: &str) -> Option<T> {
        let held = self.0.read().await;
        held.get(key).map(|(_, value)| value.clone())
    }

    pub(super) async fn store(&self, key: &str, value: T) {
        let mut held = self.0.write().await;
        held.insert(key.to_owned(), (Instant::now(), value));
    }
}
