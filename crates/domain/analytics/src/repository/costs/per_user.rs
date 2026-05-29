//! Per-user cost queries for `CostAnalyticsRepository`.
//!
//! Scopes spend, token, and conversation reads from `ai_requests` to a single
//! [`UserId`], including model/agent breakdowns and recent-context summaries
//! used for per-user billing and usage views.

use super::CostAnalyticsRepository;
use crate::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ContextId, UserId};

use crate::models::cli::{
    ContextGroupRow, ContextSummaryRow, CostBreakdownRow, CostSummaryRow, PreviousCostRow,
    RecentContextRow,
};

impl CostAnalyticsRepository {
    pub async fn get_summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow> {
        sqlx::query_as!(
            CostSummaryRow,
            r#"
            SELECT
                COUNT(*)::bigint as "requests!",
                SUM(cost_microdollars)::bigint as "cost",
                SUM(tokens_used)::bigint as "tokens"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2 AND user_id = $3
            "#,
            start,
            end,
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_previous_cost_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow> {
        sqlx::query_as!(
            PreviousCostRow,
            r#"
            SELECT SUM(cost_microdollars)::bigint as "cost"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2 AND user_id = $3
            "#,
            start,
            end,
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_breakdown_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            SELECT
                model as "name!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                COUNT(*)::bigint as "requests!",
                COALESCE(SUM(tokens_used), 0)::bigint as "tokens!"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2 AND user_id = $4
            GROUP BY model
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit,
            user_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_context_summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ContextSummaryRow> {
        sqlx::query_as!(
            ContextSummaryRow,
            r#"
            SELECT
                COUNT(DISTINCT context_id)::bigint as "conversations!",
                COUNT(*)::bigint as "ai_requests!"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
              AND user_id = $3
              AND context_id IS NOT NULL
            "#,
            start,
            end,
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_contexts_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>> {
        sqlx::query_as!(
            ContextGroupRow,
            r#"
            SELECT
                model as "name!",
                COUNT(DISTINCT context_id)::bigint as "conversations!",
                COUNT(*)::bigint as "ai_requests!"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
              AND user_id = $3
              AND context_id IS NOT NULL
            GROUP BY model
            ORDER BY COUNT(DISTINCT context_id) DESC
            LIMIT $4
            "#,
            start,
            end,
            user_id.as_str(),
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_contexts_by_agent_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>> {
        sqlx::query_as!(
            ContextGroupRow,
            r#"
            SELECT
                COALESCE(at.agent_name, 'unattributed') as "name!",
                COUNT(DISTINCT r.context_id)::bigint as "conversations!",
                COUNT(*)::bigint as "ai_requests!"
            FROM ai_requests r
            LEFT JOIN agent_tasks at ON at.task_id = r.task_id
            WHERE r.created_at >= $1 AND r.created_at < $2
              AND r.user_id = $3
              AND r.context_id IS NOT NULL
            GROUP BY COALESCE(at.agent_name, 'unattributed')
            ORDER BY COUNT(DISTINCT r.context_id) DESC
            LIMIT $4
            "#,
            start,
            end,
            user_id.as_str(),
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_recent_contexts_for_user(
        &self,
        user_id: &UserId,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<RecentContextRow>> {
        sqlx::query_as!(
            RecentContextRow,
            r#"
            SELECT
                ctx.context_id as "context_id!: ContextId",
                ctx.last_activity as "last_activity!",
                ctx.ai_requests as "ai_requests!",
                last_req.model,
                last_task.agent_name
            FROM (
                SELECT
                    r.context_id,
                    MAX(r.created_at) AS last_activity,
                    COUNT(*) AS ai_requests
                FROM ai_requests r
                WHERE r.user_id = $1
                  AND r.created_at < $2
                  AND r.context_id IS NOT NULL
                GROUP BY r.context_id
                ORDER BY MAX(r.created_at) DESC
                LIMIT $3
            ) ctx
            LEFT JOIN LATERAL (
                SELECT model, task_id FROM ai_requests
                WHERE context_id = ctx.context_id
                ORDER BY created_at DESC
                LIMIT 1
            ) last_req ON TRUE
            LEFT JOIN agent_tasks last_task ON last_task.task_id = last_req.task_id
            ORDER BY ctx.last_activity DESC
            "#,
            user_id.as_str(),
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
