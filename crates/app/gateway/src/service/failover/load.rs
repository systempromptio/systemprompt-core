//! The in-process in-flight gauge `least_busy` selection reads.
//!
//! One counter per deployment, keyed by provider and upstream model. A send
//! holds an [`InFlight`] guard from the moment it leaves until the upstream
//! answers (headers for a stream), so the gauge measures concurrent upstream
//! calls in this process, not cluster-wide load.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// Process-wide in-flight counters, one per deployment.
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
    pub fn global() -> &'static Self {
        static LOAD: OnceLock<DeploymentLoad> = OnceLock::new();
        LOAD.get_or_init(Self::default)
    }

    #[must_use]
    pub fn key(provider: &str, upstream_model: &str) -> String {
        format!("{provider}/{upstream_model}")
    }

    fn counter(&self, key: &str) -> Arc<AtomicU64> {
        let mut map = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
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
