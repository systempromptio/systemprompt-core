//! Repository for conversational contexts (multi-turn dialogue state).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod message;
mod mutations;
pub mod notifications;
mod queries;

pub use notifications::ContextNotificationRepository;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::error::AgentError;

#[derive(Debug, Clone)]
pub struct ContextRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    db_pool: DbPool,
}

impl ContextRepository {
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
