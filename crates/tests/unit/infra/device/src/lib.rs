//! Unit tests for the systemprompt-device crate, through its public API.
//!
//! Test structure mirrors the source module structure:
//! - Source: `crates/infra/device/src/secure/`
//! - Test: `crates/tests/unit/infra/device/src/secure/`

#[cfg(test)]
mod accounts;
#[cfg(test)]
mod client;
#[cfg(test)]
mod protocol;
#[cfg(test)]
mod secure;
#[cfg(test)]
mod view_tests;
