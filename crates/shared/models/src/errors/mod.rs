//! Cross-cutting error types for `systemprompt-models`.
//!
//! This module hosts `thiserror`-derived enums returned by the public
//! surface of this crate. Public APIs never return `anyhow::Error`; they
//! convert to one of the typed enums declared here. Downstream entry
//! crates that use `anyhow` (`entry/cli`, `entry/api`) continue to consume
//! these errors transparently via `?` because every enum implements
//! `std::error::Error`.
//!
//! Public re-exports:
//!
//! - [`ParseEnumError`], [`ConfigError`] — string parsing failures.
//! - [`ConfigValidationError`] — services / agents / plugins validation.
//! - [`RowParseError`] — JSON-row deserialization failures.
//! - [`MetadataError`] — MCP `_meta` payload decoding.
//! - [`SecretsError`] — on-disk secrets document.
//! - [`ProviderError`] / [`ProviderResult`] — plug-in trait abstractions.
//! - [`CoreError`] — legacy umbrella enum with HTTP status mapping.
//! - [`ServiceError`] — application-tier umbrella enum.

pub use systemprompt_traits::RepositoryError;

mod core;
pub mod macros;
mod metadata;
mod parse;
mod provider;
mod row;
mod secrets;
mod service;
mod validation;

pub use core::CoreError;
pub use metadata::MetadataError;
pub use parse::{ConfigError, ParseEnumError};
pub use provider::{ProviderError, ProviderResult};
pub use row::RowParseError;
pub use secrets::SecretsError;
pub use service::ServiceError;
pub use validation::ConfigValidationError;
