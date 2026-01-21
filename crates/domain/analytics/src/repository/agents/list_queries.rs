use anyhow::Result;
use chrono::{DateTime, Utc};

use super::AgentAnalyticsRepository;
use crate::models::cli::AgentListRow;

impl AgentAnalyticsRepository {
    pub async fn list_agents(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        sort_order: &str,
    ) -> Result<Vec<AgentListRow>> {
        match sort_order {
            "success_rate" => self.list_by_success_rate(start, end, limit).await,
            "cost" => self.list_by_cost(start, end, limit).await,
            "last_active" => self.list_by_last_active(start, end, limit).await,
            _ => self.list_by_task_count(start, end, limit).await,
        }
    }

    async fn list_by_success_rate(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListRow>> {
        sqlx::query_as!(
            AgentListRow,
            r#"
            SELECT
                t.agent_name as "agent_name!",
                COUNT(*)::bigint as "task_count!",
                COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                MAX(t.started_at) as "last_active!"
            FROM agent_tasks t
            LEFT JOIN ai_requests r ON r.task_id = t.task_id
            WHERE t.started_at >= $1 AND t.started_at < $2
              AND t.agent_name IS NOT NULL
            GROUP BY t.agent_name
            ORDER BY CASE WHEN COUNT(*) > 0
                THEN COUNT(*) FILTER (WHERE t.status = 'completed')::float / COUNT(*)::float
                ELSE 0 END DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    async fn list_by_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListRow>> {
        sqlx::query_as!(
            AgentListRow,
            r#"
            SELECT
                t.agent_name as "agent_name!",
                COUNT(*)::bigint as "task_count!",
                COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                MAX(t.started_at) as "last_active!"
            FROM agent_tasks t
            LEFT JOIN ai_requests r ON r.task_id = t.task_id
            WHERE t.started_at >= $1 AND t.started_at < $2
              AND t.agent_name IS NOT NULL
            GROUP BY t.agent_name
            ORDER BY COALESCE(SUM(r.cost_cents), 0) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    async fn list_by_last_active(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListRow>> {
        sqlx::query_as!(
            AgentListRow,
            r#"
            SELECT
                t.agent_name as "agent_name!",
                COUNT(*)::bigint as "task_count!",
                COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                MAX(t.started_at) as "last_active!"
            FROM agent_tasks t
            LEFT JOIN ai_requests r ON r.task_id = t.task_id
            WHERE t.started_at >= $1 AND t.started_at < $2
              AND t.agent_name IS NOT NULL
            GROUP BY t.agent_name
            ORDER BY MAX(t.started_at) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    async fn list_by_task_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListRow>> {
        sqlx::query_as!(
            AgentListRow,
            r#"
            SELECT
                t.agent_name as "agent_name!",
                COUNT(*)::bigint as "task_count!",
                COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                MAX(t.started_at) as "last_active!"
            FROM agent_tasks t
            LEFT JOIN ai_requests r ON r.task_id = t.task_id
            WHERE t.started_at >= $1 AND t.started_at < $2
              AND t.agent_name IS NOT NULL
            GROUP BY t.agent_name
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
    }
}
