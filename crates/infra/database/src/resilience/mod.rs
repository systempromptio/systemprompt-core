//! Composable resilience primitives for outbound calls.
//!
//! This module wraps fallible async operations (HTTP calls to AI providers,
//! RPCs to MCP servers, the crate's own connection and transaction retries) so
//! a slow or failing dependency cannot pin workers or cascade into a
//! platform-wide outage. It is deliberately domain-agnostic: every primitive is
//! generic over a caller-supplied error type and an error [`classify`]r, so it
//! can be reused by `systemprompt-ai`, `systemprompt-mcp`, or any other caller
//! without depending on their error enums.
//!
//! # Primitives
//!
//! - [`retry::retry_async`] — bounded exponential backoff with full jitter;
//!   honors a `Retry-After` hint and never retries [`Outcome::Permanent`]
//!   failures.
//! - [`breaker::CircuitBreaker`] — `Closed` → `Open` → `HalfOpen` state machine
//!   that fast-fails while a dependency is unhealthy.
//! - [`bulkhead::Bulkhead`] — a non-blocking concurrency cap; a saturated
//!   dependency rejects callers instead of queueing them.
//! - [`guard::ResilienceGuard`] — composes bulkhead → breaker → retry →
//!   per-attempt timeout into a single `execute` call.
//! - [`stream::guarded_stream`] — bounds each poll of a streaming response with
//!   an idle timeout and holds a bulkhead permit for the stream's lifetime.
//!
//! # Error model
//!
//! Callers classify their own errors via a `Fn(&E) -> Outcome` closure.
//! Failures the guard itself produces (circuit open, bulkhead full, timeout)
//! surface as [`error::ResilienceError`], which the caller maps back into its
//! domain error.

pub mod breaker;
pub mod bulkhead;
pub mod classify;
pub mod config;
pub mod error;
pub mod guard;
pub mod retry;
pub mod stream;

pub use breaker::CircuitBreaker;
pub use bulkhead::Bulkhead;
pub use classify::Outcome;
pub use config::{BreakerConfig, BulkheadConfig, ResilienceConfig, RetryConfig};
pub use error::ResilienceError;
pub use guard::ResilienceGuard;
pub use retry::retry_async;
pub use stream::guarded_stream;
