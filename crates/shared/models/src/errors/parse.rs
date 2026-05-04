//! Parsing-layer error types: enum tag dispatch and config bootstrap.

/// Failure to parse a string into one of the enum types defined by this
/// crate (audience, permission, role, hook event, transport binding…).
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("invalid {kind}: {value}")]
pub struct ParseEnumError {
    /// Human-readable name of the enum that failed to parse (e.g.
    /// `"permission"`).
    pub kind: &'static str,
    /// The offending input string.
    pub value: String,
}

impl ParseEnumError {
    /// Construct a new `ParseEnumError` for the given enum kind and input.
    #[must_use]
    pub fn new(kind: &'static str, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

/// Failure to bootstrap or read the global `Config` singleton.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ConfigError {
    /// `Config::init()` has not yet been called.
    #[error("Config not initialized. Call Config::init() first.")]
    NotInitialized,

    /// The supplied database URL is not a `PostgreSQL` connection string.
    #[error("DATABASE_URL must be a PostgreSQL connection string (postgres:// or postgresql://)")]
    InvalidPostgresUrl,
}
