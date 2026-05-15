//! Cost analytics repository.
//!
//! Aggregates microdollar-precision AI request costs from `ai_requests`.
//! [`platform`] holds platform-wide rollups; [`per_user`] holds the
//! user-scoped cost and conversation-context queries.

mod per_user;
mod platform;

use crate::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct CostAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl CostAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
