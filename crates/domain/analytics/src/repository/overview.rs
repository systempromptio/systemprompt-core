//! Top-line dashboard metrics across all analytics domains.
//!
//! [`OverviewAnalyticsRepository`] reads the headline counts shown on the
//! analytics overview — conversations, agent/task activity, AI requests,
//! tool executions, sessions, and cost — each scoped to a time window.
//! Request, tool and task figures come from the hourly rollups, so a window
//! is widened to whole UTC hours at its start: every hour that begins before
//! `end` and does not end before `start` counts in full.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::ContextKind;

use crate::models::reporting::{
    OverviewAgentRow, OverviewCostRow, OverviewRequestRow, OverviewToolRow,
};

#[derive(Debug)]
pub struct OverviewAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl OverviewAnalyticsRepository {
    pub fn new(db: &DbPool) -> Self {
        let pool = db.pool();
        Self { pool }
    }

    pub async fn get_conversation_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM report_user_contexts WHERE created_at >= $1 AND created_at < $2 AND kind = $3"#,
            start,
            end,
            ContextKind::User.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_agent_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OverviewAgentRow> {
        sqlx::query_as!(
            OverviewAgentRow,
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
        .await
        .map_err(Into::into)
    }

    pub async fn get_request_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OverviewRequestRow> {
        sqlx::query_as!(
            OverviewRequestRow,
            r#"
            SELECT
                COALESCE(SUM(request_count), 0)::bigint as "total!",
                SUM(tokens)::bigint as "total_tokens",
                (SUM(latency_sum_ms)::numeric / NULLIF(SUM(latency_n), 0))::float8 as "avg_latency"
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

    pub async fn get_tool_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OverviewToolRow> {
        sqlx::query_as!(
            OverviewToolRow,
            r#"
            SELECT
                COALESCE(SUM(execution_count), 0)::bigint as "total!",
                COALESCE(SUM(execution_count) FILTER (WHERE status = 'success'), 0)::bigint as "successful!"
            FROM report_tool_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND server_name NOT IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode')
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_active_session_count(&self, since: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::bigint as "count!"
            FROM report_clean_traffic
            WHERE ended_at IS NULL
              AND last_activity_at >= $1            "#,
            since
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_total_session_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM report_clean_traffic WHERE started_at >= $1 AND started_at < $2"#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OverviewCostRow> {
        sqlx::query_as!(
            OverviewCostRow,
            r#"
            SELECT SUM(cost_microdollars)::bigint as "cost"
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
}
