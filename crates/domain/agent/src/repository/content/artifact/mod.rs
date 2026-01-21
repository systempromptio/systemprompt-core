mod mutations;
mod parts;
mod queries;

pub use parts::{get_artifact_parts, persist_artifact_part};

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::{Repository as RepositoryTrait, RepositoryError};

#[derive(Debug, Clone)]
pub struct ArtifactRepository {
    db_pool: DbPool,
}

impl RepositoryTrait for ArtifactRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}

impl ArtifactRepository {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub(crate) fn get_pg_pool(&self) -> Result<Arc<PgPool>, RepositoryError> {
        self.db_pool
            .as_ref()
            .get_postgres_pool()
            .ok_or_else(|| RepositoryError::Database("PostgreSQL pool not available".to_string()))
    }
}
