pub mod message;
mod mutations;
mod queries;

use crate::repository::Repository;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::RepositoryError;

#[derive(Debug, Clone)]
pub struct ContextRepository {
    db_pool: DbPool,
}

impl ContextRepository {
    #[must_use]
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    fn get_pg_pool(&self) -> Result<Arc<PgPool>, RepositoryError> {
        self.db_pool
            .as_ref()
            .get_postgres_pool()
            .ok_or_else(|| RepositoryError::InvalidData("PostgreSQL pool not available".to_string()))
    }
}

impl Repository for ContextRepository {
    fn pool(&self) -> &DbPool {
        &self.db_pool
    }
}

impl systemprompt_traits::Repository for ContextRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}
