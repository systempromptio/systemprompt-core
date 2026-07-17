//! Per-agent aggregate statistics queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};

use super::AgentAnalyticsRepository;
use crate::models::cli::{AgentAiStatsRow, AgentStatsRow, AgentTaskRow};

impl AgentAnalyticsRepository {
    pub async fn get_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<AgentStatsRow> {
        if let Some(agent) = agent_filter {
            let pattern = format!("%{}%", agent);
            sqlx::query_as!(
                AgentStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT agent_name)::bigint as "total_agents!",
                    COUNT(*)::bigint as "total_tasks!",
                    -- agent_tasks.status stores the canonical A2A TaskState value
                    -- written by task_state_to_db_string (TASK_STATE_*), which the
                    -- agent_tasks_status_check CHECK constraint also enforces.
                    COUNT(*) FILTER (WHERE status = 'TASK_STATE_COMPLETED')::bigint as "completed_tasks!",
                    COUNT(*) FILTER (WHERE status = 'TASK_STATE_FAILED')::bigint as "failed_tasks!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_execution_time_ms!"
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                  AND agent_name ILIKE $3
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
                AgentStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT agent_name)::bigint as "total_agents!",
                    COUNT(*)::bigint as "total_tasks!",
                    COUNT(*) FILTER (WHERE status = 'TASK_STATE_COMPLETED')::bigint as "completed_tasks!",
                    COUNT(*) FILTER (WHERE status = 'TASK_STATE_FAILED')::bigint as "failed_tasks!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_execution_time_ms!"
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                "#,
                start,
                end
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }

    pub async fn get_ai_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentAiStatsRow> {
        sqlx::query_as!(
            AgentAiStatsRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total_ai_requests!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars!"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_tasks_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<Vec<AgentTaskRow>> {
        if let Some(agent) = agent_filter {
            let pattern = format!("%{}%", agent);
            sqlx::query_as!(
                AgentTaskRow,
                r#"
                SELECT
                    started_at as "started_at!",
                    status,
                    execution_time_ms
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                  AND agent_name ILIKE $3
                ORDER BY started_at
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
                AgentTaskRow,
                r#"
                SELECT
                    started_at as "started_at!",
                    status,
                    execution_time_ms
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                ORDER BY started_at
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
