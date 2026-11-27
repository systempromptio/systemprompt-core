//! Unit tests for the systemprompt-mobile crate.
//!
//! Tests cover:
//! - `InstanceAddress` parsing: which addresses name an instance
//! - `MobileError` as the mapping of every client failure
//! - `Instances::discover` against a stub instance

#[cfg(test)]
mod address;
#[cfg(test)]
mod discovery;
#[cfg(test)]
mod error;
