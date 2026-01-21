use async_trait::async_trait;

#[async_trait]
pub trait Repository: Send + Sync {
    type Pool;
    type Error: std::error::Error + Send + Sync + 'static;

    fn pool(&self) -> &Self::Pool;
}

#[async_trait]
pub trait CrudRepository<T>: Repository {
    type Id;

    async fn create(&self, entity: T) -> Result<T, Self::Error>;
    async fn get(&self, id: Self::Id) -> Result<Option<T>, Self::Error>;
    async fn update(&self, entity: T) -> Result<T, Self::Error>;
    async fn delete(&self, id: Self::Id) -> Result<(), Self::Error>;
    async fn list(&self) -> Result<Vec<T>, Self::Error>;
}

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
