//! Errors raised while loading or validating the on-disk secrets file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("{context}: {source}")]
    Parse {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("{0}")]
    Invalid(String),
}
