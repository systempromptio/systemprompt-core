//! Per-tool detail queries: summary statistics, the status breakdown, top
//! error messages, the execution series for trend charts, and the overview's
//! tool totals. The existence check, the status breakdown and the overview
//! totals read the hourly rollup (`report_tool_usage_hourly`), widening the
//! window to whole UTC hours at its start. The rest stay on the per-execution
//! view, because a p95, a grouping by message text and a per-execution series
//! cannot be added up from a rollup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_models::mcp::McpToolExecutionStatus;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::mcp::{
    ToolErrorRow, ToolExecutionRow, ToolOverviewMetrics, ToolStatsRow, ToolStatusBreakdownRow,
    ToolSummaryRow,
};

use super::ToolUsageReportRepository;

impl ToolUsageReportRepository {
    pub(super) async fn stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<ToolStatsRow, RepositoryError> {
        if let Some(tool) = tool_filter {
            let pattern = format!("%{tool}%");
            sqlx::query_as!(
                ToolStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT tool_name)::bigint as "total_tools!",
                    COUNT(*)::bigint as "total_executions!",
                    COUNT(*) FILTER (WHERE status = $4)::bigint as "successful!",
                    COUNT(*) FILTER (WHERE status = $5)::bigint as "failed!",
                    COUNT(*) FILTER (WHERE status = $6)::bigint as "timeout!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                    COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms)::float8, 0) as "p95_time!"
                FROM report_mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2 AND tool_name ILIKE $3
                "#,
                start, end, pattern,
                McpToolExecutionStatus::Success.as_str(),
                McpToolExecutionStatus::Failed.as_str(),
                McpToolExecutionStatus::Timeout.as_str()
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(RepositoryError::from)
        } else {
            sqlx::query_as!(
                ToolStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT tool_name)::bigint as "total_tools!",
                    COUNT(*)::bigint as "total_executions!",
                    COUNT(*) FILTER (WHERE status = $3)::bigint as "successful!",
                    COUNT(*) FILTER (WHERE status = $4)::bigint as "failed!",
                    COUNT(*) FILTER (WHERE status = $5)::bigint as "timeout!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                    COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms)::float8, 0) as "p95_time!"
                FROM report_mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2
                "#,
                start, end,
                McpToolExecutionStatus::Success.as_str(),
                McpToolExecutionStatus::Failed.as_str(),
                McpToolExecutionStatus::Timeout.as_str()
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(RepositoryError::from)
        }
    }

    pub(super) async fn tool_execution_count(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        let pattern = format!("%{tool_filter}%");
        let count = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(execution_count), 0)::bigint as "count!" FROM report_tool_usage_hourly WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND tool_name ILIKE $3"#,
            start,
            end,
            pattern
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(count)
    }

    pub(super) async fn tool_summary(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolSummaryRow, RepositoryError> {
        let pattern = format!("%{tool_filter}%");
        sqlx::query_as!(
            ToolSummaryRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total!",
                COUNT(*) FILTER (WHERE status = $4)::bigint as "successful!",
                COUNT(*) FILTER (WHERE status = $5)::bigint as "failed!",
                COUNT(*) FILTER (WHERE status = $6)::bigint as "timeout!",
                COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms)::float8, 0) as "p95_time!"
            FROM report_mcp_tool_executions
            WHERE tool_name ILIKE $1 AND created_at >= $2 AND created_at < $3
            "#,
            pattern, start, end,
            McpToolExecutionStatus::Success.as_str(),
            McpToolExecutionStatus::Failed.as_str(),
            McpToolExecutionStatus::Timeout.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    pub(super) async fn status_breakdown(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolStatusBreakdownRow>, RepositoryError> {
        let pattern = format!("%{tool_filter}%");
        sqlx::query_as!(
            ToolStatusBreakdownRow,
            r#"
            SELECT status as "status!", SUM(execution_count)::bigint as "status_count!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND tool_name ILIKE $3
            GROUP BY status
            ORDER BY 2 DESC
            "#,
            start,
            end,
            pattern
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    pub(super) async fn top_errors(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolErrorRow>, RepositoryError> {
        let pattern = format!("%{tool_filter}%");
        sqlx::query_as!(
            ToolErrorRow,
            r#"
            SELECT
                COALESCE(SUBSTRING(error_message FROM 1 FOR 100), 'Unknown error') as "error_msg",
                COUNT(*)::bigint as "error_count!"
            FROM report_mcp_tool_executions
            WHERE tool_name ILIKE $1 AND created_at >= $2 AND created_at < $3 AND status = $4
            GROUP BY SUBSTRING(error_message FROM 1 FOR 100)
            ORDER BY 2 DESC
            LIMIT 10
            "#,
            pattern,
            start,
            end,
            McpToolExecutionStatus::Failed.as_str()
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    pub(super) async fn executions_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<Vec<ToolExecutionRow>, RepositoryError> {
        if let Some(tool) = tool_filter {
            let pattern = format!("%{tool}%");
            sqlx::query_as!(
                ToolExecutionRow,
                r#"
                SELECT
                    created_at as "created_at!",
                    status,
                    execution_time_ms
                FROM report_mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2 AND tool_name ILIKE $3
                ORDER BY created_at
                "#,
                start,
                end,
                pattern
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(RepositoryError::from)
        } else {
            sqlx::query_as!(
                ToolExecutionRow,
                r#"
                SELECT
                    created_at as "created_at!",
                    status,
                    execution_time_ms
                FROM report_mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2
                ORDER BY created_at
                "#,
                start,
                end
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(RepositoryError::from)
        }
    }

    pub(super) async fn overview_tool_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolOverviewMetrics, RepositoryError> {
        sqlx::query_as!(
            ToolOverviewMetrics,
            r#"
            SELECT
                COALESCE(SUM(execution_count), 0)::bigint as "total!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "successful!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode')
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }
}
