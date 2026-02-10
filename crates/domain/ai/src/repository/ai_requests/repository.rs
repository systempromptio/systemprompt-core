use crate::error::RepositoryError;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TraceId, UserId};

#[derive(Debug)]
pub struct CreateAiRequest<'a> {
    pub user_id: &'a UserId,
    pub session_id: Option<&'a SessionId>,
    pub trace_id: Option<&'a TraceId>,
    pub provider: &'a str,
    pub model: &'a str,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct AiRequestRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl AiRequestRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let pool = db
            .pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { pool, write_pool })
    }

    pub(super) fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub(super) fn write_pool(&self) -> &PgPool {
        &self.write_pool
    }
}
