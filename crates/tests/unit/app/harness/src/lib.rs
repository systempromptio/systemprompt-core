//! Unit tests for the systemprompt-harness crate, through its public API.
//!
//! Test structure mirrors the source module structure:
//! - Source: `crates/app/harness/src/projection/`
//! - Test: `crates/tests/unit/app/harness/src/projection/`

#[cfg(test)]
mod command;
#[cfg(test)]
mod extension;
#[cfg(test)]
mod governance;
#[cfg(test)]
mod members;
#[cfg(test)]
mod projection;
#[cfg(test)]
mod sync;
