//! [`ResilienceGuard`] — composes bulkhead, breaker, retry and timeout.

use std::fmt;
use std::future::Future;

use tokio::sync::OwnedSemaphorePermit;

use super::breaker::CircuitBreaker;
use super::bulkhead::Bulkhead;
use super::classify::Outcome;
use super::config::ResilienceConfig;
use super::error::ResilienceError;
use super::retry::retry_async;

/// Wraps a caller error so a per-attempt timeout can flow through the retry
/// loop as a transient failure without the caller's `E` needing a timeout
/// variant.
enum AttemptError<E> {
    Inner(E),
    Timeout(std::time::Duration),
}

impl<E: fmt::Display> fmt::Display for AttemptError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inner(err) => write!(f, "{err}"),
            Self::Timeout(after) => write!(f, "attempt timed out after {after:?}"),
        }
    }
}

/// The resilience policy applied to one logical dependency.
///
/// One guard instance is shared across all calls to that dependency (an AI
/// provider, an MCP server) so its breaker and bulkhead state accumulate across
/// calls.
#[derive(Debug)]
pub struct ResilienceGuard {
    key: String,
    cfg: ResilienceConfig,
    breaker: CircuitBreaker,
    bulkhead: Bulkhead,
}

impl ResilienceGuard {
    pub fn new(key: impl Into<String>, cfg: ResilienceConfig) -> Self {
        let key = key.into();
        let breaker = CircuitBreaker::new(key.clone(), cfg.breaker);
        let bulkhead = Bulkhead::new(key.clone(), cfg.bulkhead.max_concurrent);
        Self {
            key,
            cfg,
            breaker,
            bulkhead,
        }
    }

    /// The dependency key this guard protects.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The policy this guard applies.
    #[must_use]
    pub const fn config(&self) -> &ResilienceConfig {
        &self.cfg
    }

    /// The circuit breaker, exposed so out-of-band signals (a health monitor)
    /// can report failures and successes directly.
    #[must_use]
    pub const fn breaker(&self) -> &CircuitBreaker {
        &self.breaker
    }

    /// Run `op` under the full policy: bulkhead admission → breaker admission →
    /// retry loop, each attempt bounded by `request_timeout`.
    pub async fn execute<T, E, F, Fut>(
        &self,
        classify: impl Fn(&E) -> Outcome + Send + Sync,
        op: F,
    ) -> Result<T, ResilienceError<E>>
    where
        T: Send,
        E: std::error::Error + Send,
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, E>> + Send,
    {
        let _permit = self.acquire_permit::<E>()?;
        let timeout = self.cfg.request_timeout;

        let classify_attempt = |err: &AttemptError<E>| match err {
            AttemptError::Timeout(_) => Outcome::Transient { retry_after: None },
            AttemptError::Inner(inner) => classify(inner),
        };
        let attempt = || async {
            match tokio::time::timeout(timeout, op()).await {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(err)) => Err(AttemptError::Inner(err)),
                Err(_) => Err(AttemptError::Timeout(timeout)),
            }
        };

        match retry_async(&self.cfg.retry, &self.key, classify_attempt, attempt).await {
            Ok(value) => {
                self.breaker.record_success();
                Ok(value)
            },
            Err(AttemptError::Inner(err)) => {
                self.breaker.record_failure();
                Err(ResilienceError::Inner(err))
            },
            Err(AttemptError::Timeout(after)) => {
                self.breaker.record_failure();
                Err(ResilienceError::Timeout { after })
            },
        }
    }

    /// Admit one call: a bulkhead permit plus breaker admission.
    ///
    /// Used directly by streaming callers, which hold the returned permit for
    /// the stream's lifetime and report the outcome via [`Self::breaker`].
    /// Non-streaming callers should use [`Self::execute`] instead.
    pub fn acquire_permit<E>(&self) -> Result<OwnedSemaphorePermit, ResilienceError<E>>
    where
        E: std::error::Error,
    {
        let permit = self
            .bulkhead
            .try_acquire()
            .map_err(|_| ResilienceError::BulkheadFull {
                key: self.key.clone(),
                limit: self.bulkhead.limit(),
            })?;
        self.breaker
            .acquire()
            .map_err(|_| ResilienceError::CircuitOpen {
                key: self.key.clone(),
            })?;
        Ok(permit)
    }
}
