//! MCP tool-execution analytics over `mcp_tool_executions`.
//!
//! [`ToolAnalyticsRepository`] reports tool success/failure/timeout rates,
//! latency percentiles, error and per-agent breakdowns, and trend series.
//! Cross-tool listing lives in `list_queries`, single-tool drill-down in
//! `detail_queries`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod detail_queries;
pub(super) mod list_queries;

use crate::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct ToolAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl ToolAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
