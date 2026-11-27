//! Unit tests for the systemprompt-drive crate, through its public API.
//!
//! The tests of the Drive member of the device composition are in
//! `crates/tests/unit/app/harness/src/members/drive/`.

#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod index_tests;
#[cfg(test)]
mod names_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod sync_conflict_tests;
#[cfg(test)]
mod sync_pull_tests;
#[cfg(test)]
mod sync_push_tests;
#[cfg(test)]
mod sync_tests;
#[cfg(test)]
mod upload_tests;
