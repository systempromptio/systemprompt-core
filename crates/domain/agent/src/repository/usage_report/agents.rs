//! Per-agent statistics, the overview and the tool calls
//! each agent made. Totals read the hourly task and AI usage rollups, widened
//! to whole UTC hours at the window's start; the trend samples and top errors
//! are per task and read `report_agent_tasks`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_models::a2a::TaskState;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::agent::{
    AgentAiStatsRow, AgentOverviewRow, AgentStatsRow, AgentTaskSample, ToolAgentUsageRow,
};

use super::{AgentActivityReportRepository, contains_pattern};
use crate::models::rows::{TaskSampleDbRow, ToolCallerDbRow};

impl AgentActivityReportRepository {
    pub(super) async fn stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<AgentStatsRow, RepositoryError> {
        if let Some(agent) = agent_filter {
            let pattern = contains_pattern(agent);
            Ok(sqlx::query_as!(
                AgentStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT agent_name)::bigint as "total_agents!",
                    COALESCE(SUM(task_count), 0)::bigint as "total_tasks!",
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
            .await?)
        } else {
            Ok(sqlx::query_as!(
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
            .await?)
        }
    }

    pub(super) async fn ai_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentAiStatsRow, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentAiStatsRow,
            r#"
            SELECT
                COALESCE(SUM(request_count), 0)::bigint as "total_ai_requests!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars!"
            FROM report_agent_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?)
    }

    pub(super) async fn samples(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<Vec<AgentTaskSample>, RepositoryError> {
        let rows = if let Some(agent) = agent_filter {
            let pattern = contains_pattern(agent);
            sqlx::query_as!(
                TaskSampleDbRow,
                r#"
                SELECT
                    started_at as "started_at!",
                    status as "status: TaskState",
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
            .await?
        } else {
            sqlx::query_as!(
                TaskSampleDbRow,
                r#"
                SELECT
                    started_at as "started_at!",
                    status as "status: TaskState",
                    execution_time_ms
                FROM report_agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                ORDER BY started_at
                "#,
                start,
                end
            )
            .fetch_all(&*self.pool)
            .await?
        };
        Ok(rows.into_iter().map(AgentTaskSample::from).collect())
    }

    pub(super) async fn overview(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentOverviewRow, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentOverviewRow,
            r#"
            SELECT
                COUNT(DISTINCT NULLIF(agent_name, ''))::bigint as "active_agents!",
                COALESCE(SUM(task_count), 0)::bigint as "total_tasks!",
                COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0)::bigint as "completed_tasks!"
            FROM report_agent_task_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?)
    }

    pub(super) async fn tool_callers(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolAgentUsageRow>, RepositoryError> {
        let pattern = contains_pattern(tool_filter);
        let rows = sqlx::query_as!(
            ToolCallerDbRow,
            r#"
            SELECT
                tu.agent_name as "agent_name!",
                SUM(tu.execution_count)::bigint as "usage_count!"
            FROM report_agent_tool_usage_hourly tu
            WHERE tu.tool_name ILIKE $1
              AND tu.bucket >= date_trunc('hour', $2::timestamptz, 'UTC') AND tu.bucket < $3
            GROUP BY tu.agent_name
            ORDER BY 2 DESC
            LIMIT 10
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows.into_iter().map(ToolAgentUsageRow::from).collect())
    }
}
