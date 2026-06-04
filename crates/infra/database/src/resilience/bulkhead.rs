//! A non-blocking concurrency limiter.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Clone, Copy)]
pub struct Full;

#[derive(Debug)]
pub struct Bulkhead {
    key: String,
    limit: usize,
    semaphore: Arc<Semaphore>,
}

impl Bulkhead {
    pub fn new(key: impl Into<String>, max_concurrent: usize) -> Self {
        Self {
            key: key.into(),
            limit: max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    // The returned permit must be held for the call's duration (and, for
    // streaming responses, the stream's lifetime).
    pub fn try_acquire(&self) -> Result<OwnedSemaphorePermit, Full> {
        Arc::clone(&self.semaphore)
            .try_acquire_owned()
            .map_err(|_e| {
                tracing::warn!(
                    key = %self.key,
                    limit = self.limit,
                    "bulkhead saturated, rejecting call",
                );
                Full
            })
    }

    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }
}
