//! Parsing-layer error types: enum tag dispatch and config bootstrap.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("invalid {kind}: {value}")]
pub struct ParseEnumError {
    pub kind: &'static str,
    pub value: String,
}

impl ParseEnumError {
    #[must_use]
    pub fn new(kind: &'static str, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ConfigError {
    #[error("Config not initialized. Call Config::init() first.")]
    NotInitialized,

    #[error("DATABASE_URL must be a PostgreSQL connection string (postgres:// or postgresql://)")]
    InvalidPostgresUrl,
}
