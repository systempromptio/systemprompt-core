//! The errors of reading extension state and extension configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::{DuplicateDependency, MissingDependency};
use systemprompt_traits::BoxedSource;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtensionStateError {
    #[error("Extension state has no value of the requested type: {0}")]
    Missing(#[from] MissingDependency),

    #[error("Extension state refused a provider: {0}")]
    DuplicateProvider(#[from] DuplicateDependency),

    #[error("Extension state is read before the host bound it")]
    NotReady,

    #[error("Extension state handle is already bound")]
    AlreadyBound,

    #[error("Failed to build provided state: {0}")]
    Provider(#[source] BoxedSource),
}

#[derive(Debug, Error)]
pub enum ExtensionConfigError {
    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidValue { key: String, message: String },

    #[error("Failed to parse configuration: {source}")]
    ParseError {
        #[source]
        source: BoxedSource,
    },

    #[error("Schema validation failed: {0}")]
    SchemaValidation(#[source] BoxedSource),
}
