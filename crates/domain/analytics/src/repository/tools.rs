use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;

use crate::models::cli::{
    ToolAgentUsageRow, ToolErrorRow, ToolExecutionRow, ToolListRow, ToolStatsRow,
    ToolStatusBreakdownRow, ToolSummaryRow,
};

#[derive(Debug)]
pub struct ToolAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl ToolAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_tools(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        server_filter: Option<&str>,
        sort_order: &str,
    ) -> Result<Vec<ToolListRow>> {
        if let Some(server) = server_filter {
            let pattern = format!("%{}%", server);
            match sort_order {
                "success_rate" => sqlx::query_as!(
                    ToolListRow,
                    r#"
                        SELECT
                            tool_name as "tool_name!",
                            server_name as "server_name!",
                            COUNT(*)::bigint as "execution_count!",
                            COUNT(*) FILTER (WHERE status = 'success')::bigint as "success_count!",
                            COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                            MAX(created_at) as "last_used!"
                        FROM mcp_tool_executions
                        WHERE created_at >= $1 AND created_at < $2
                          AND server_name ILIKE $3
                        GROUP BY tool_name, server_name
                        ORDER BY CASE WHEN COUNT(*) > 0
                            THEN COUNT(*) FILTER (WHERE status = 'success')::float / COUNT(*)::float
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
                .map_err(Into::into),
                "avg_time" => sqlx::query_as!(
                    ToolListRow,
                    r#"
                        SELECT
                            tool_name as "tool_name!",
                            server_name as "server_name!",
                            COUNT(*)::bigint as "execution_count!",
                            COUNT(*) FILTER (WHERE status = 'success')::bigint as "success_count!",
                            COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                            MAX(created_at) as "last_used!"
                        FROM mcp_tool_executions
                        WHERE created_at >= $1 AND created_at < $2
                          AND server_name ILIKE $3
                        GROUP BY tool_name, server_name
                        ORDER BY COALESCE(AVG(execution_time_ms), 0) DESC
                        LIMIT $4
                        "#,
                    start,
                    end,
                    pattern,
                    limit
                )
                .fetch_all(&*self.pool)
                .await
                .map_err(Into::into),
                _ => {
                    // Default: execution_count
                    sqlx::query_as!(
                        ToolListRow,
                        r#"
                        SELECT
                            tool_name as "tool_name!",
                            server_name as "server_name!",
                            COUNT(*)::bigint as "execution_count!",
                            COUNT(*) FILTER (WHERE status = 'success')::bigint as "success_count!",
                            COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                            MAX(created_at) as "last_used!"
                        FROM mcp_tool_executions
                        WHERE created_at >= $1 AND created_at < $2
                          AND server_name ILIKE $3
                        GROUP BY tool_name, server_name
                        ORDER BY COUNT(*) DESC
                        LIMIT $4
                        "#,
                        start,
                        end,
                        pattern,
                        limit
                    )
                    .fetch_all(&*self.pool)
                    .await
                    .map_err(Into::into)
                },
            }
        } else {
            match sort_order {
                "success_rate" => sqlx::query_as!(
                    ToolListRow,
                    r#"
                        SELECT
                            tool_name as "tool_name!",
                            server_name as "server_name!",
                            COUNT(*)::bigint as "execution_count!",
                            COUNT(*) FILTER (WHERE status = 'success')::bigint as "success_count!",
                            COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                            MAX(created_at) as "last_used!"
                        FROM mcp_tool_executions
                        WHERE created_at >= $1 AND created_at < $2
                        GROUP BY tool_name, server_name
                        ORDER BY CASE WHEN COUNT(*) > 0
                            THEN COUNT(*) FILTER (WHERE status = 'success')::float / COUNT(*)::float
                            ELSE 0 END DESC
                        LIMIT $3
                        "#,
                    start,
                    end,
                    limit
                )
                .fetch_all(&*self.pool)
                .await
                .map_err(Into::into),
                "avg_time" => sqlx::query_as!(
                    ToolListRow,
                    r#"
                        SELECT
                            tool_name as "tool_name!",
                            server_name as "server_name!",
                            COUNT(*)::bigint as "execution_count!",
                            COUNT(*) FILTER (WHERE status = 'success')::bigint as "success_count!",
                            COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                            MAX(created_at) as "last_used!"
                        FROM mcp_tool_executions
                        WHERE created_at >= $1 AND created_at < $2
                        GROUP BY tool_name, server_name
                        ORDER BY COALESCE(AVG(execution_time_ms), 0) DESC
                        LIMIT $3
                        "#,
                    start,
                    end,
                    limit
                )
                .fetch_all(&*self.pool)
                .await
                .map_err(Into::into),
                _ => {
                    // Default: execution_count
                    sqlx::query_as!(
                        ToolListRow,
                        r#"
                        SELECT
                            tool_name as "tool_name!",
                            server_name as "server_name!",
                            COUNT(*)::bigint as "execution_count!",
                            COUNT(*) FILTER (WHERE status = 'success')::bigint as "success_count!",
                            COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                            MAX(created_at) as "last_used!"
                        FROM mcp_tool_executions
                        WHERE created_at >= $1 AND created_at < $2
                        GROUP BY tool_name, server_name
                        ORDER BY COUNT(*) DESC
                        LIMIT $3
                        "#,
                        start,
                        end,
                        limit
                    )
                    .fetch_all(&*self.pool)
                    .await
                    .map_err(Into::into)
                },
            }
        }
    }

    pub async fn get_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<ToolStatsRow> {
        if let Some(tool) = tool_filter {
            let pattern = format!("%{}%", tool);
            sqlx::query_as!(
                ToolStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT tool_name)::bigint as "total_tools!",
                    COUNT(*)::bigint as "total_executions!",
                    COUNT(*) FILTER (WHERE status = 'success')::bigint as "successful!",
                    COUNT(*) FILTER (WHERE status = 'failed')::bigint as "failed!",
                    COUNT(*) FILTER (WHERE status = 'timeout')::bigint as "timeout!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                    COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms)::float8, 0) as "p95_time!"
                FROM mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2
                  AND tool_name ILIKE $3
                "#,
                start,
                end,
                pattern
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                ToolStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT tool_name)::bigint as "total_tools!",
                    COUNT(*)::bigint as "total_executions!",
                    COUNT(*) FILTER (WHERE status = 'success')::bigint as "successful!",
                    COUNT(*) FILTER (WHERE status = 'failed')::bigint as "failed!",
                    COUNT(*) FILTER (WHERE status = 'timeout')::bigint as "timeout!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                    COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms)::float8, 0) as "p95_time!"
                FROM mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2
                "#,
                start,
                end
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }

    pub async fn tool_exists(
        &self,
        tool_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let pattern = format!("%{}%", tool_name);
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM mcp_tool_executions WHERE tool_name ILIKE $1 AND created_at >= \
             $2 AND created_at < $3",
        )
        .bind(&pattern)
        .bind(start)
        .bind(end)
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn get_tool_summary(
        &self,
        tool_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolSummaryRow> {
        let pattern = format!("%{}%", tool_name);
        sqlx::query_as!(
            ToolSummaryRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total!",
                COUNT(*) FILTER (WHERE status = 'success')::bigint as "successful!",
                COUNT(*) FILTER (WHERE status = 'failed')::bigint as "failed!",
                COUNT(*) FILTER (WHERE status = 'timeout')::bigint as "timeout!",
                COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!",
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms)::float8, 0) as "p95_time!"
            FROM mcp_tool_executions
            WHERE tool_name ILIKE $1
              AND created_at >= $2 AND created_at < $3
            "#,
            pattern,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_status_breakdown(
        &self,
        tool_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolStatusBreakdownRow>> {
        let pattern = format!("%{}%", tool_name);
        sqlx::query_as!(
            ToolStatusBreakdownRow,
            r#"
            SELECT status as "status!", COUNT(*)::bigint as "status_count!"
            FROM mcp_tool_executions
            WHERE tool_name ILIKE $1
              AND created_at >= $2 AND created_at < $3
            GROUP BY status
            ORDER BY 2 DESC
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_errors(
        &self,
        tool_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolErrorRow>> {
        let pattern = format!("%{}%", tool_name);
        sqlx::query_as!(
            ToolErrorRow,
            r#"
            SELECT
                COALESCE(SUBSTRING(error_message FROM 1 FOR 100), 'Unknown error') as "error_msg",
                COUNT(*)::bigint as "error_count!"
            FROM mcp_tool_executions
            WHERE tool_name ILIKE $1
              AND created_at >= $2 AND created_at < $3
              AND status = 'failed'
            GROUP BY SUBSTRING(error_message FROM 1 FOR 100)
            ORDER BY 2 DESC
            LIMIT 10
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_usage_by_agent(
        &self,
        tool_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolAgentUsageRow>> {
        let pattern = format!("%{}%", tool_name);
        sqlx::query_as!(
            ToolAgentUsageRow,
            r#"
            SELECT
                COALESCE(at.agent_name, CASE WHEN mte.task_id IS NULL THEN 'Direct Call' ELSE 'Unlinked Task' END) as "agent_name",
                COUNT(*)::bigint as "usage_count!"
            FROM mcp_tool_executions mte
            LEFT JOIN agent_tasks at ON at.task_id = mte.task_id
            WHERE mte.tool_name ILIKE $1
              AND mte.created_at >= $2 AND mte.created_at < $3
            GROUP BY COALESCE(at.agent_name, CASE WHEN mte.task_id IS NULL THEN 'Direct Call' ELSE 'Unlinked Task' END)
            ORDER BY 2 DESC
            LIMIT 10
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_executions_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<Vec<ToolExecutionRow>> {
        if let Some(tool) = tool_filter {
            let pattern = format!("%{}%", tool);
            sqlx::query_as!(
                ToolExecutionRow,
                r#"
                SELECT
                    created_at as "created_at!",
                    status,
                    execution_time_ms
                FROM mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2
                  AND tool_name ILIKE $3
                ORDER BY created_at
                "#,
                start,
                end,
                pattern
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                ToolExecutionRow,
                r#"
                SELECT
                    created_at as "created_at!",
                    status,
                    execution_time_ms
                FROM mcp_tool_executions
                WHERE created_at >= $1 AND created_at < $2
                ORDER BY created_at
                "#,
                start,
                end
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }
}
