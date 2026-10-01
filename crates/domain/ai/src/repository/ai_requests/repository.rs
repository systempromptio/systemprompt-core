//! `AiRequestRepository` construction and shared helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[must_use]
#[derive(Debug, Clone)]
pub struct AiRequestRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl AiRequestRepository {
    pub fn new(db: &DbPool) -> Self {
        let pool = db.pool();
        let write_pool = db.write_pool();
        Self { pool, write_pool }
    }

    pub(super) fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub(super) fn write_pool(&self) -> &PgPool {
        &self.write_pool
    }
}
