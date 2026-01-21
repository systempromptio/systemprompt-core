use anyhow::Result;
use chrono::{DateTime, Utc};

use super::ToolAnalyticsRepository;
use crate::models::cli::ToolListRow;

impl ToolAnalyticsRepository {
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
            self.list_tools_with_filter(start, end, limit, &pattern, sort_order)
                .await
        } else {
            self.list_tools_unfiltered(start, end, limit, sort_order)
                .await
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_tools_with_filter(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        pattern: &str,
        sort_order: &str,
    ) -> Result<Vec<ToolListRow>> {
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
                WHERE created_at >= $1 AND created_at < $2 AND server_name ILIKE $3
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
                WHERE created_at >= $1 AND created_at < $2 AND server_name ILIKE $3
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
            _ => sqlx::query_as!(
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
                WHERE created_at >= $1 AND created_at < $2 AND server_name ILIKE $3
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
            .map_err(Into::into),
        }
    }

    async fn list_tools_unfiltered(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        sort_order: &str,
    ) -> Result<Vec<ToolListRow>> {
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
            _ => sqlx::query_as!(
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
            .map_err(Into::into),
        }
    }
}
