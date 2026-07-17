//! Artifact repository — persistence of binary/structured outputs produced by
//! tasks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod converters;
mod mutations;
mod parts;
mod queries;

pub use parts::{get_artifact_parts, persist_artifact_part};

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::error::AgentError;

#[derive(Debug, Clone)]
pub struct ArtifactRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ArtifactRepository {
    pub fn new(db: &DbPool) -> Result<Self, AgentError> {
        let pool = db.pool_arc().map_err(|e| AgentError::Init(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| AgentError::Init(e.to_string()))?;
        Ok(Self { pool, write_pool })
    }
}
