//! The in-process in-flight gauge `least_busy` selection reads.
//!
//! One counter per breaker key: `provider/endpoint`, and for a hop billed to a
//! member that member too, so one member's concurrency never steers another's
//! selection. An idle counter reads zero, the same as one that was never
//! created, so idle counters are dropped once the registry passes the
//! breakers' cap. A
//! send holds an [`InFlight`] guard from the moment it leaves until the
//! upstream answers (headers for a stream), so the gauge measures concurrent
//! upstream calls in this process, not cluster-wide load.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use super::breakers::REGISTRY_CAP;

/// In-flight counters owned by one gateway, one per deployment.
#[derive(Debug, Default)]
pub struct DeploymentLoad {
    inner: Mutex<HashMap<String, Arc<AtomicU64>>>,
}

/// Holds one in-flight slot on a deployment until dropped.
#[derive(Debug)]
pub struct InFlight(Arc<AtomicU64>);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl DeploymentLoad {
    fn counter(&self, key: &str) -> Arc<AtomicU64> {
        let mut map = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= REGISTRY_CAP && !map.contains_key(key) {
            map.retain(|_, counter| {
                Arc::strong_count(counter) > 1 || counter.load(Ordering::Relaxed) > 0
            });
        }
        Arc::clone(map.entry(key.to_owned()).or_default())
    }

    #[must_use]
    pub fn in_flight(&self, key: &str) -> u64 {
        self.counter(key).load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn enter(&self, key: &str) -> InFlight {
        let counter = self.counter(key);
        counter.fetch_add(1, Ordering::Relaxed);
        InFlight(counter)
    }
}
