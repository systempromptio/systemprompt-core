//! Generic repository error type used by domain crates.

/// Errors returned by repository implementations.
///
/// Repositories from any domain crate can convert their backend-specific
/// failures into [`RepositoryError`] using
/// [`RepositoryError::database`]. The `Other` variant provides an escape
/// hatch for `anyhow`-based call sites that have not yet been migrated to
/// typed errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RepositoryError {
    /// Database driver or pool failure.
    #[error("database error: {0}")]
    Database(Box<dyn std::error::Error + Send + Sync>),

    /// Lookup returned no row when one was required.
    #[error("entity not found: {0}")]
    NotFound(String),

    /// JSON (de)serialization around stored payloads failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Persisted data did not match the expected shape.
    #[error("invalid data: {0}")]
    InvalidData(String),

    /// A database constraint (unique, foreign key, ...) was violated.
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    /// Adapter for legacy `anyhow`-based call sites.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl RepositoryError {
    /// Wrap an arbitrary backend error in [`RepositoryError::Database`].
    pub fn database(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Database(Box::new(err))
    }
}
