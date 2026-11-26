//! Per-agent listings ordered by task count, success rate, cost or recency.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::agent::{AgentListRow, AgentSortOrder};

use super::AgentActivityReportRepository;
use crate::models::rows::AgentListDbRow;

impl AgentActivityReportRepository {
    pub(super) async fn list_agents_sorted(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        sort: AgentSortOrder,
    ) -> Result<Vec<AgentListRow>, RepositoryError> {
        let rows = match sort {
            AgentSortOrder::SuccessRate => self.list_by_success_rate(start, end, limit).await,
            AgentSortOrder::Cost => self.list_by_cost(start, end, limit).await,
            AgentSortOrder::LastActive => self.list_by_last_active(start, end, limit).await,
            AgentSortOrder::TaskCount => self.list_by_task_count(start, end, limit).await,
        }?;
        Ok(rows.into_iter().map(AgentListRow::from).collect())
    }

    async fn list_by_success_rate(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListDbRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentListDbRow,
            r#"
            WITH tasks AS (
                SELECT agent_name, SUM(task_count) AS task_count,
                       COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0) AS completed_count,
                       SUM(exec_sum_ms) AS exec_sum_ms, SUM(exec_n) AS exec_n,
                       MAX(last_started_at) AS last_active
                FROM report_agent_task_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            ), costs AS (
                SELECT agent_name, SUM(cost_microdollars) AS cost
                FROM report_agent_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            )
            SELECT
                t.agent_name as "agent_name!",
                t.task_count::bigint as "task_count!",
                t.completed_count::bigint as "completed_count!",
                COALESCE(t.exec_sum_ms::numeric / NULLIF(t.exec_n, 0), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(c.cost, 0)::bigint as "total_cost_microdollars!",
                t.last_active as "last_active!"
            FROM tasks t
            LEFT JOIN costs c ON c.agent_name = t.agent_name
            ORDER BY CASE WHEN t.task_count > 0
                THEN t.completed_count::float / t.task_count::float
                ELSE 0 END DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    async fn list_by_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListDbRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentListDbRow,
            r#"
            WITH tasks AS (
                SELECT agent_name, SUM(task_count) AS task_count,
                       COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0) AS completed_count,
                       SUM(exec_sum_ms) AS exec_sum_ms, SUM(exec_n) AS exec_n,
                       MAX(last_started_at) AS last_active
                FROM report_agent_task_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            ), costs AS (
                SELECT agent_name, SUM(cost_microdollars) AS cost
                FROM report_agent_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            )
            SELECT
                t.agent_name as "agent_name!",
                t.task_count::bigint as "task_count!",
                t.completed_count::bigint as "completed_count!",
                COALESCE(t.exec_sum_ms::numeric / NULLIF(t.exec_n, 0), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(c.cost, 0)::bigint as "total_cost_microdollars!",
                t.last_active as "last_active!"
            FROM tasks t
            LEFT JOIN costs c ON c.agent_name = t.agent_name
            ORDER BY COALESCE(c.cost, 0) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    async fn list_by_last_active(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListDbRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentListDbRow,
            r#"
            WITH tasks AS (
                SELECT agent_name, SUM(task_count) AS task_count,
                       COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0) AS completed_count,
                       SUM(exec_sum_ms) AS exec_sum_ms, SUM(exec_n) AS exec_n,
                       MAX(last_started_at) AS last_active
                FROM report_agent_task_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            ), costs AS (
                SELECT agent_name, SUM(cost_microdollars) AS cost
                FROM report_agent_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            )
            SELECT
                t.agent_name as "agent_name!",
                t.task_count::bigint as "task_count!",
                t.completed_count::bigint as "completed_count!",
                COALESCE(t.exec_sum_ms::numeric / NULLIF(t.exec_n, 0), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(c.cost, 0)::bigint as "total_cost_microdollars!",
                t.last_active as "last_active!"
            FROM tasks t
            LEFT JOIN costs c ON c.agent_name = t.agent_name
            ORDER BY t.last_active DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    async fn list_by_task_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AgentListDbRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentListDbRow,
            r#"
            WITH tasks AS (
                SELECT agent_name, SUM(task_count) AS task_count,
                       COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0) AS completed_count,
                       SUM(exec_sum_ms) AS exec_sum_ms, SUM(exec_n) AS exec_n,
                       MAX(last_started_at) AS last_active
                FROM report_agent_task_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            ), costs AS (
                SELECT agent_name, SUM(cost_microdollars) AS cost
                FROM report_agent_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> ''
                GROUP BY agent_name
            )
            SELECT
                t.agent_name as "agent_name!",
                t.task_count::bigint as "task_count!",
                t.completed_count::bigint as "completed_count!",
                COALESCE(t.exec_sum_ms::numeric / NULLIF(t.exec_n, 0), 0)::bigint as "avg_execution_time_ms!",
                COALESCE(c.cost, 0)::bigint as "total_cost_microdollars!",
                t.last_active as "last_active!"
            FROM tasks t
            LEFT JOIN costs c ON c.agent_name = t.agent_name
            ORDER BY t.task_count DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?)
    }
}
