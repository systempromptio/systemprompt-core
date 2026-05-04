//! Artifact repository — persistence of binary/structured outputs produced by
//! tasks.

mod converters;
mod mutations;
mod parts;
mod queries;

pub use parts::{get_artifact_parts, persist_artifact_part};

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::error::AgentError;

/// Repository for task artifacts; holds the read and write pools.
#[derive(Debug, Clone)]
pub struct ArtifactRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ArtifactRepository {
    /// Construct a new `ArtifactRepository` from a shared [`DbPool`].
    ///
    /// # Errors
    /// Returns [`AgentError::Init`] if the read or write pool cannot be
    /// acquired.
    pub fn new(db: &DbPool) -> Result<Self, AgentError> {
        let pool = db.pool_arc().map_err(|e| AgentError::Init(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| AgentError::Init(e.to_string()))?;
        Ok(Self { pool, write_pool })
    }
}
