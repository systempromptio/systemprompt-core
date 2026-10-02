//! AI provider error type and result alias.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::BoxedSource;

pub type AiProviderResult<T> = Result<T, AiProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiProviderError {
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Internal error: {0}")]
    Internal(#[source] BoxedSource),
}
