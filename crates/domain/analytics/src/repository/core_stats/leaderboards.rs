use anyhow::Result;

use super::CoreStatsRepository;
use crate::models::{TopAgent, TopTool, TopUser};

impl CoreStatsRepository {
    pub async fn get_top_users(&self, limit: i64) -> Result<Vec<TopUser>> {
        sqlx::query_as!(
            TopUser,
            r#"
            SELECT
                u.id as user_id,
                u.name as user_name,
                COUNT(DISTINCT s.session_id) as "session_count!",
                COUNT(DISTINCT t.task_id) as "task_count!",
                COUNT(DISTINCT a.request_id) as "ai_request_count!",
                COALESCE(SUM(a.cost_cents)::float / 100.0, 0.0) as "total_cost!"
            FROM users u
            LEFT JOIN user_sessions s ON s.user_id = u.id
            LEFT JOIN agent_tasks t ON t.user_id = u.id
            LEFT JOIN ai_requests a ON a.user_id = u.id
            WHERE u.status NOT IN ('deleted', 'temporary') AND NOT ('anonymous' = ANY(u.roles))
            GROUP BY u.id, u.name
            ORDER BY "ai_request_count!" DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_agents(&self, limit: i64) -> Result<Vec<TopAgent>> {
        sqlx::query_as!(
            TopAgent,
            r#"
            SELECT
                agent_name as "agent_name!",
                COUNT(*) as "task_count!",
                COALESCE(
                    COUNT(*) FILTER (WHERE status = 'completed')::float / NULLIF(COUNT(*), 0),
                    0.0
                ) as "success_rate!",
                COALESCE(AVG(EXTRACT(EPOCH FROM (updated_at - created_at)) * 1000)::bigint, 0) as "avg_duration_ms!"
            FROM agent_tasks
            WHERE agent_name IS NOT NULL
            GROUP BY agent_name
            ORDER BY "task_count!" DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_tools(&self, limit: i64) -> Result<Vec<TopTool>> {
        sqlx::query_as!(
            TopTool,
            r#"
            SELECT
                tool_name,
                COUNT(*) as "execution_count!",
                COALESCE(
                    COUNT(*) FILTER (WHERE status = 'success')::float / NULLIF(COUNT(*), 0),
                    0.0
                ) as "success_rate!",
                COALESCE(AVG(execution_time_ms), 0)::bigint as "avg_duration_ms!"
            FROM mcp_tool_executions
            GROUP BY tool_name
            ORDER BY "execution_count!" DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
