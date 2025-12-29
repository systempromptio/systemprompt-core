//! Unit tests for systemprompt-traits crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/shared/traits/src/auth.rs`
//! - Test: `crates/tests/unit/shared/traits/src/auth.rs`

#[cfg(test)]
mod auth;

#[cfg(test)]
mod db_value;

#[cfg(test)]
mod extension_error;

#[cfg(test)]
mod validation;

#[cfg(test)]
mod validation_report;

#[cfg(test)]
mod startup_events;
