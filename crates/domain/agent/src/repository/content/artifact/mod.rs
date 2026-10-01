//! Artifact repository — persistence of binary/structured outputs produced by
//! tasks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod converters;
mod mutations;
mod parts;
mod queries;

pub(crate) use converters::artifact_from_row;
pub use parts::{get_artifact_parts, persist_artifact_part};

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct ArtifactRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ArtifactRepository {
    pub fn new(db: &DbPool) -> Self {
        let pool = db.pool();
        let write_pool = db.write_pool();
        Self { pool, write_pool }
    }
}
