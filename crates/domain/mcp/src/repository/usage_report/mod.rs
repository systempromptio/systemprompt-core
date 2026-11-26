//! The tool-usage report repository: answers the shared
//! [`ToolUsageReport`] port over this member's report views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod detail;
mod list;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::mcp::{
    ToolErrorRow, ToolExecutionRow, ToolListParams, ToolListRow, ToolOverviewMetrics, ToolStatsRow,
    ToolStatusBreakdownRow, ToolSummaryRow, ToolUsageReport,
};

#[derive(Debug, Clone)]
pub struct ToolUsageReportRepository {
    pool: Arc<PgPool>,
}

impl ToolUsageReportRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }
}

#[async_trait]
impl ToolUsageReport for ToolUsageReportRepository {
    async fn list_tools(
        &self,
        params: ToolListParams<'_>,
    ) -> Result<Vec<ToolListRow>, RepositoryError> {
        self.list(params).await
    }

    async fn stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<ToolStatsRow, RepositoryError> {
        Self::stats(self, start, end, tool_filter).await
    }

    async fn tool_execution_count(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        Self::tool_execution_count(self, tool_filter, start, end).await
    }

    async fn tool_summary(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolSummaryRow, RepositoryError> {
        Self::tool_summary(self, tool_filter, start, end).await
    }

    async fn status_breakdown(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolStatusBreakdownRow>, RepositoryError> {
        Self::status_breakdown(self, tool_filter, start, end).await
    }

    async fn top_errors(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolErrorRow>, RepositoryError> {
        Self::top_errors(self, tool_filter, start, end).await
    }

    async fn executions_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<Vec<ToolExecutionRow>, RepositoryError> {
        Self::executions_for_trends(self, start, end, tool_filter).await
    }

    async fn overview_tool_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolOverviewMetrics, RepositoryError> {
        Self::overview_tool_metrics(self, start, end).await
    }
}
