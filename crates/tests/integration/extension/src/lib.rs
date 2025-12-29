//! Tests for the systemprompt-extension framework.
//!
//! This crate contains all tests for the extension framework, following
//! the project's testing policy of keeping tests in separate crates.

#[cfg(test)]
mod any_tests;

#[cfg(test)]
mod builder_tests;

#[cfg(test)]
mod capability_tests;

#[cfg(test)]
mod context_tests;

#[cfg(test)]
mod error_tests;

#[cfg(test)]
mod extension_tests;

#[cfg(test)]
mod hlist_tests;

#[cfg(test)]
mod registry_tests;

#[cfg(test)]
mod typed_api_tests;

#[cfg(test)]
mod typed_config_tests;

#[cfg(test)]
mod typed_schema_tests;

#[cfg(test)]
mod types_tests;
