//! The tool leaderboard over the hourly rollup (`report_tool_usage_hourly`),
//! filtered by server or not, in each sort order. A host-native tool a
//! client hook reported (Bash, Read, …) is recorded under the vantage
//! point's own name as its server; those pseudo-servers are not MCP tools and
//! stay out of every list. The window widens to whole UTC hours at its start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::mcp::{ToolListParams, ToolListRow, ToolListSort};

use super::ToolUsageReportRepository;
use crate::models::rows::ToolListDbRow;

impl ToolUsageReportRepository {
    pub(super) async fn list(
        &self,
        params: ToolListParams<'_>,
    ) -> Result<Vec<ToolListRow>, RepositoryError> {
        let ToolListParams {
            start,
            end,
            limit,
            server_filter,
            sort,
        } = params;
        let rows = if let Some(server) = server_filter {
            let pattern = format!("%{server}%");
            match sort {
                ToolListSort::SuccessRate => {
                    self.filtered_by_success_rate(start, end, &pattern, limit)
                        .await
                },
                ToolListSort::AvgTime => {
                    self.filtered_by_avg_time(start, end, &pattern, limit).await
                },
                ToolListSort::ExecutionCount => {
                    self.filtered_by_count(start, end, &pattern, limit).await
                },
            }
        } else {
            match sort {
                ToolListSort::SuccessRate => {
                    self.unfiltered_by_success_rate(start, end, limit).await
                },
                ToolListSort::AvgTime => self.unfiltered_by_avg_time(start, end, limit).await,
                ToolListSort::ExecutionCount => self.unfiltered_by_count(start, end, limit).await,
            }
        }?;
        Ok(rows.into_iter().map(ToolListRow::from).collect())
    }

    async fn filtered_by_success_rate(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        pattern: &str,
        limit: i64,
    ) -> Result<Vec<ToolListDbRow>, RepositoryError> {
        sqlx::query_as!(
            ToolListDbRow,
            r#"
            SELECT
                tool_name as "tool_name!",
                server_name as "server_name!",
                SUM(execution_count)::bigint as "execution_count!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "success_count!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!",
                MAX(last_at) as "last_used!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND server_name ILIKE $3
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')
            GROUP BY tool_name, server_name
            ORDER BY CASE WHEN SUM(execution_count) > 0
                THEN COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::float / SUM(execution_count)::float
                ELSE 0 END DESC
            LIMIT $4
            "#,
            start,
            end,
            pattern,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    async fn filtered_by_avg_time(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        pattern: &str,
        limit: i64,
    ) -> Result<Vec<ToolListDbRow>, RepositoryError> {
        sqlx::query_as!(
            ToolListDbRow,
            r#"
            SELECT
                tool_name as "tool_name!",
                server_name as "server_name!",
                SUM(execution_count)::bigint as "execution_count!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "success_count!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!",
                MAX(last_at) as "last_used!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND server_name ILIKE $3
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')
            GROUP BY tool_name, server_name
            ORDER BY COALESCE(SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0), 0) DESC
            LIMIT $4
            "#,
            start,
            end,
            pattern,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    async fn filtered_by_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        pattern: &str,
        limit: i64,
    ) -> Result<Vec<ToolListDbRow>, RepositoryError> {
        sqlx::query_as!(
            ToolListDbRow,
            r#"
            SELECT
                tool_name as "tool_name!",
                server_name as "server_name!",
                SUM(execution_count)::bigint as "execution_count!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "success_count!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!",
                MAX(last_at) as "last_used!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND server_name ILIKE $3
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')
            GROUP BY tool_name, server_name
            ORDER BY SUM(execution_count) DESC
            LIMIT $4
            "#,
            start,
            end,
            pattern,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    async fn unfiltered_by_success_rate(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ToolListDbRow>, RepositoryError> {
        sqlx::query_as!(
            ToolListDbRow,
            r#"
            SELECT
                tool_name as "tool_name!",
                server_name as "server_name!",
                SUM(execution_count)::bigint as "execution_count!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "success_count!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!",
                MAX(last_at) as "last_used!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')
            GROUP BY tool_name, server_name
            ORDER BY CASE WHEN SUM(execution_count) > 0
                THEN COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::float / SUM(execution_count)::float
                ELSE 0 END DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    async fn unfiltered_by_avg_time(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ToolListDbRow>, RepositoryError> {
        sqlx::query_as!(
            ToolListDbRow,
            r#"
            SELECT
                tool_name as "tool_name!",
                server_name as "server_name!",
                SUM(execution_count)::bigint as "execution_count!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "success_count!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!",
                MAX(last_at) as "last_used!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')
            GROUP BY tool_name, server_name
            ORDER BY COALESCE(SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0), 0) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    async fn unfiltered_by_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ToolListDbRow>, RepositoryError> {
        sqlx::query_as!(
            ToolListDbRow,
            r#"
            SELECT
                tool_name as "tool_name!",
                server_name as "server_name!",
                SUM(execution_count)::bigint as "execution_count!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "success_count!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!",
                MAX(last_at) as "last_used!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')
            GROUP BY tool_name, server_name
            ORDER BY SUM(execution_count) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::from)
    }
}
