//! Unit tests for systemprompt-core-analytics crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/analytics/src/error.rs`
//! - Test: `crates/tests/unit/domain/analytics/src/error.rs`

#[cfg(test)]
mod error;

#[cfg(test)]
mod models;

#[cfg(test)]
mod services;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod projection;

#[cfg(test)]
mod resource_metrics;

#[cfg(test)]
mod feedback_facts;
