//! Unit tests for the `systemprompt-database` `resilience` module.
//!
//! Tests cover:
//! - `retry_async` backoff, `Retry-After` honoring, and permanent-failure
//!   short-circuit
//! - `CircuitBreaker` open / half-open / recovery transitions
//! - `Bulkhead` admission and rejection
//! - `guarded_stream` pass-through and idle-timeout abort
//! - `ResilienceGuard` end-to-end composition

#![allow(clippy::all)]

#[cfg(test)]
mod breaker;
#[cfg(test)]
mod bulkhead;
#[cfg(test)]
mod classify;
#[cfg(test)]
mod config;
#[cfg(test)]
mod error;
#[cfg(test)]
mod guard;
#[cfg(test)]
mod retry;
#[cfg(test)]
mod stream;
