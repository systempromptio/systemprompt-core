//! Top-line dashboard metrics across all analytics domains.
//!
//! [`OverviewAnalyticsRepository`] reads the headline counts shown on the
//! analytics overview — conversations, agent/task activity, AI requests,
//! tool executions, sessions, and cost — each scoped to a time window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::ContextKind;

use crate::models::cli::{OverviewAgentRow, OverviewCostRow, OverviewRequestRow, OverviewToolRow};

#[derive(Debug)]
pub struct OverviewAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl OverviewAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_conversation_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM user_contexts WHERE created_at >= $1 AND created_at < $2 AND kind = $3"#,
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
                COUNT(DISTINCT agent_name)::bigint as "active_agents!",
                COUNT(*)::bigint as "total_tasks!",
                COUNT(*) FILTER (WHERE status = 'TASK_STATE_COMPLETED')::bigint as "completed_tasks!"
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

    pub async fn get_request_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OverviewRequestRow> {
        sqlx::query_as!(
            OverviewRequestRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total!",
                SUM(tokens_used)::bigint as "total_tokens",
                AVG(latency_ms)::float8 as "avg_latency"
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

    pub async fn get_tool_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OverviewToolRow> {
        sqlx::query_as!(
            OverviewToolRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total!",
                COUNT(*) FILTER (WHERE status = 'success')::bigint as "successful!"
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

    pub async fn get_active_session_count(&self, since: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::bigint as "count!"
            FROM v_clean_traffic
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
            r#"SELECT COUNT(*)::bigint as "count!" FROM v_clean_traffic WHERE started_at >= $1 AND started_at < $2"#,
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
}
