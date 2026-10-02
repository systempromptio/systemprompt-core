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
//! - [`ParseEnumError`], [`GlobalConfigError`] — string parsing failures.
//! - [`ServicesValidationError`] — services / agents / plugins validation.
//! - [`MetadataError`] — MCP `_meta` payload decoding.
//! - [`SecretsError`] — on-disk secrets document.
//! - [`AiInferenceError`] / [`McpRegistryError`] — the typed errors of the
//!   dyn-dispatched provider seams.
//! - [`RepositoryError`] — the workspace's single repository error, defined in
//!   `systemprompt-traits`; it renders over HTTP through
//!   [`crate::api::ApiError`]'s `From` impl.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_traits::RepositoryError;

pub mod macros;
mod metadata;
mod parse;
mod provider;
mod secrets;
mod validation;

pub use metadata::MetadataError;
pub use parse::{GlobalConfigError, ParseEnumError};
pub use provider::{AiInferenceError, AiInferenceResult, McpRegistryError, McpRegistryResult};
pub use secrets::SecretsError;
pub use validation::ServicesValidationError;
