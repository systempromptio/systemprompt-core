//! A non-blocking concurrency limiter.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Returned by [`Bulkhead::try_acquire`] when the concurrency limit is reached.
#[derive(Debug, Clone, Copy)]
pub struct Full;

/// A concurrency cap for one dependency.
///
/// Acquisition is non-blocking: when the limit is reached the call is rejected
/// with [`Full`] rather than queued, so a slow dependency fast-fails callers
/// instead of letting them pile up and exhaust workers.
#[derive(Debug)]
pub struct Bulkhead {
    key: String,
    limit: usize,
    semaphore: Arc<Semaphore>,
}

impl Bulkhead {
    /// Create a bulkhead admitting `cfg.max_concurrent` in-flight calls.
    pub fn new(key: impl Into<String>, max_concurrent: usize) -> Self {
        Self {
            key: key.into(),
            limit: max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Try to admit a call. The returned permit must be held for the call's
    /// duration (including, for streaming responses, the stream's lifetime).
    pub fn try_acquire(&self) -> Result<OwnedSemaphorePermit, Full> {
        Arc::clone(&self.semaphore)
            .try_acquire_owned()
            .map_err(|_| {
                tracing::warn!(
                    key = %self.key,
                    limit = self.limit,
                    "bulkhead saturated, rejecting call",
                );
                Full
            })
    }

    /// The configured concurrency limit.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }
}
