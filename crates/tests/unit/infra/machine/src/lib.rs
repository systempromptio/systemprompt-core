//! Unit tests for the systemprompt-machine crate, through its public API.
//!
//! Test structure mirrors the source module structure:
//! - Source: `crates/infra/machine/src/scheduler/`
//! - Test: `crates/tests/unit/infra/machine/src/scheduler/`
//!
//! No test registers a scheduler entry, requests elevation or writes outside
//! its own values: the scheduler tests cover the documents each system is
//! handed, never the hand-over.

#[cfg(test)]
mod elevation_tests;
#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod scheduler;
#[cfg(test)]
mod support;
