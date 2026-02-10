#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(Box<dyn std::error::Error + Send + Sync>),

    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl RepositoryError {
    pub fn database(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Database(Box::new(err))
    }
}
