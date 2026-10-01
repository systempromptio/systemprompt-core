//! Cost analytics repository.
//!
//! Aggregates microdollar-precision AI request costs from `ai_requests`.
//! [`platform`] holds platform-wide rollups; [`per_user`] holds the
//! user-scoped cost and conversation-context queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod per_user;
mod platform;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct CostAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl CostAnalyticsRepository {
    pub fn new(db: &DbPool) -> Self {
        let pool = db.pool();
        Self { pool }
    }

    #[must_use]
    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}
