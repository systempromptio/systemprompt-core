//! Errors raised while loading or validating the on-disk secrets file.

/// Failure to load, parse, or validate a `Secrets` document.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    /// The JSON payload could not be parsed or deserialized.
    #[error("{context}: {source}")]
    Parse {
        /// Stage at which parsing failed.
        context: &'static str,
        /// Underlying serde error.
        #[source]
        source: serde_json::Error,
    },

    /// A required field violated a length / format constraint.
    #[error("{0}")]
    Invalid(String),
}
