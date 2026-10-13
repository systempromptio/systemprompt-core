//! Per-agent aggregate statistics queries. Totals read the hourly task and
//! AI usage rollups, widened to whole UTC hours at the window's start; the
//! trend series is per task and stays on `report_agent_tasks`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};

use super::AgentAnalyticsRepository;
use crate::models::reporting::{AgentAiStatsRow, AgentStatsRow, AgentTaskRow};

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
                    COALESCE(SUM(task_count), 0)::bigint as "total_tasks!",
                    -- agent_tasks.status stores the canonical A2A TaskState value
                    -- written by task_state_to_db_string (TASK_STATE_*), which the
                    -- agent_tasks_status_check CHECK constraint also enforces.
                    COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0)::bigint as "completed_tasks!",
                    COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_FAILED'), 0)::bigint as "failed_tasks!",
                    COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_execution_time_ms!"
                FROM report_agent_task_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND agent_name <> '' AND agent_name ILIKE $3
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
                    COUNT(DISTINCT NULLIF(agent_name, ''))::bigint as "total_agents!",
                    COALESCE(SUM(task_count), 0)::bigint as "total_tasks!",
                    COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0)::bigint as "completed_tasks!",
                    COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_FAILED'), 0)::bigint as "failed_tasks!",
                    COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_execution_time_ms!"
                FROM report_agent_task_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
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
                COALESCE(SUM(request_count), 0)::bigint as "total_ai_requests!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars!"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
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
                FROM report_agent_tasks
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
                FROM report_agent_tasks
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
