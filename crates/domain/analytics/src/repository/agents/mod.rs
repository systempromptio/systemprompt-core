//! Agent analytics queries over `agent_tasks` and joined `ai_requests`.
//!
//! [`AgentAnalyticsRepository`] aggregates per-agent task counts, success
//! rates, execution time, and cost, exposing list, detail, and summary-stat
//! reads split across the sibling query submodules.

mod detail_queries;
mod list_queries;
mod stats_queries;

use crate::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct AgentAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl AgentAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
