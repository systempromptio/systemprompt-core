//! Single-agent detail reads for [`AgentActivityReportRepository`]: task
//! count, summary, status breakdown, top errors and hourly distribution.
//! Totals read the hourly task rollup widened to whole UTC hours at the
//! window's start; top errors are per task and read `report_agent_tasks`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::agent::{
    AgentErrorRow, AgentHourlyRow, AgentStatusBreakdownRow, AgentSummaryRow,
};

use super::{AgentActivityReportRepository, contains_pattern};

impl AgentActivityReportRepository {
    pub(super) async fn task_count_for(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        let pattern = contains_pattern(agent_filter);
        Ok(sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(task_count), 0)::bigint as "count!" FROM report_agent_task_hourly WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2 AND agent_name <> '' AND agent_name ILIKE $3"#,
            start,
            end,
            pattern
        )
        .fetch_one(&*self.pool)
        .await?)
    }

    pub(super) async fn summary(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentSummaryRow, RepositoryError> {
        let pattern = contains_pattern(agent_filter);
        Ok(sqlx::query_as!(
            AgentSummaryRow,
            r#"
            SELECT
                COALESCE(SUM(task_count), 0)::bigint as "total_tasks!",
                COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_COMPLETED'), 0)::bigint as "completed!",
                COALESCE(SUM(task_count) FILTER (WHERE status = 'TASK_STATE_FAILED'), 0)::bigint as "failed!",
                COALESCE((SUM(exec_sum_ms)::numeric / NULLIF(SUM(exec_n), 0))::float8, 0) as "avg_time!"
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
    }

    pub(super) async fn statuses(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentStatusBreakdownRow>, RepositoryError> {
        let pattern = contains_pattern(agent_filter);
        Ok(sqlx::query_as!(
            AgentStatusBreakdownRow,
            r#"
            SELECT status as "status!", SUM(task_count)::bigint as "status_count!"
            FROM report_agent_task_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND agent_name <> '' AND agent_name ILIKE $3
            GROUP BY status
            ORDER BY 2 DESC
            "#,
            start,
            end,
            pattern
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(super) async fn errors(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentErrorRow>, RepositoryError> {
        let pattern = contains_pattern(agent_filter);
        Ok(sqlx::query_as!(
            AgentErrorRow,
            r#"
            SELECT
                COALESCE(SUBSTRING(error_message FROM 1 FOR 100), 'Unknown error') as "error_type",
                COUNT(*)::bigint as "error_count!"
            FROM report_agent_tasks
            WHERE agent_name ILIKE $1
              AND started_at >= $2 AND started_at < $3
              AND status = 'TASK_STATE_FAILED'
            GROUP BY SUBSTRING(error_message FROM 1 FOR 100)
            ORDER BY 2 DESC
            LIMIT 10
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(super) async fn hours(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentHourlyRow>, RepositoryError> {
        let pattern = contains_pattern(agent_filter);
        Ok(sqlx::query_as!(
            AgentHourlyRow,
            r#"
            SELECT
                EXTRACT(HOUR FROM bucket)::INTEGER as "task_hour!",
                SUM(task_count)::bigint as "task_count!"
            FROM report_agent_task_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND agent_name <> '' AND agent_name ILIKE $3
            GROUP BY EXTRACT(HOUR FROM bucket)
            ORDER BY 1
            "#,
            start,
            end,
            pattern
        )
        .fetch_all(&*self.pool)
        .await?)
    }
}
