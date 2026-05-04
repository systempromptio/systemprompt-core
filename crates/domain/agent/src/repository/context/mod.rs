//! Repository for conversational contexts (multi-turn dialogue state).

pub mod message;
mod mutations;
mod queries;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::error::AgentError;

/// Repository for conversational contexts; reads from the read pool and writes
/// to the write pool.
#[derive(Debug, Clone)]
pub struct ContextRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    db_pool: DbPool,
}

impl ContextRepository {
    /// Construct a new `ContextRepository` from a shared [`DbPool`].
    ///
    /// # Errors
    /// Returns [`AgentError::Init`] if the underlying read or write pool cannot
    /// be acquired.
    pub fn new(db: &DbPool) -> Result<Self, AgentError> {
        let pool = db.pool_arc().map_err(|e| AgentError::Init(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| AgentError::Init(e.to_string()))?;
        Ok(Self {
            pool,
            write_pool,
            db_pool: Arc::clone(db),
        })
    }
}
