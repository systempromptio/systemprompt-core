//! Conversation-scoped cost queries for `CostAnalyticsRepository`.
//!
//! Distinct conversation counts per user, model and agent, the per-user
//! breakdown that carries them, and the most recent conversations of one
//! user. Every aggregate reads the per-conversation hourly rollup
//! (`report_ai_context_hourly`): a distinct count over a window counts the
//! rollup's conversation-hour rows instead of the requests behind them, and
//! the window widens to whole UTC hours like every other rollup read. User
//! and conversation names are joined afterwards by id, without an aggregate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use super::CostAnalyticsRepository;
use crate::Result;
use crate::models::RecentContextDbRow;
use crate::models::reporting::{
    ContextGroupRow, ContextSummaryRow, CostUserBreakdownRow, RecentContextRow,
};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ContextId, UserId};

impl CostAnalyticsRepository {
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
                COALESCE(SUM(request_count), 0)::bigint as "ai_requests!"
            FROM report_ai_context_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
              AND user_id = $3
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
                SUM(request_count)::bigint as "ai_requests!"
            FROM report_ai_context_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
              AND user_id = $3
              AND model <> ''
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
                COALESCE(NULLIF(agent_name, ''), 'unattributed') as "name!",
                COUNT(DISTINCT context_id)::bigint as "conversations!",
                SUM(request_count)::bigint as "ai_requests!"
            FROM report_ai_context_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
              AND user_id = $3
            GROUP BY COALESCE(NULLIF(agent_name, ''), 'unattributed')
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

    pub async fn get_breakdown_by_user(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostUserBreakdownRow>> {
        let mut rows = sqlx::query_as!(
            CostUserBreakdownRow,
            r#"
            SELECT
                user_id as "user_id!: UserId",
                NULL::text as "name?",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                SUM(request_count)::bigint as "requests!",
                COALESCE(SUM(tokens), 0)::bigint as "tokens!",
                COUNT(DISTINCT context_id)::bigint as "conversations!"
            FROM report_ai_context_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
            GROUP BY user_id
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        let ids: Vec<String> = rows.iter().map(|row| row.user_id.to_string()).collect();
        let names: HashMap<String, Option<String>> =
            sqlx::query!("SELECT id, name FROM report_users WHERE id = ANY($1)", &ids)
                .fetch_all(&*self.pool)
                .await?
                .into_iter()
                .filter_map(|row| Some((row.id?, row.name)))
                .collect();
        for row in &mut rows {
            row.name = names.get(row.user_id.as_str()).cloned().flatten();
        }
        Ok(rows)
    }

    pub async fn get_recent_contexts_for_user(
        &self,
        user_id: &UserId,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<RecentContextRow>> {
        let mut rows = sqlx::query_as!(
            RecentContextDbRow,
            r#"
            SELECT
                ctx.context_id as "context_id!: ContextId",
                ctx.last_activity as "last_activity!",
                ctx.ai_requests as "ai_requests!",
                NULLIF(last_row.model, '') as "model?",
                NULLIF(last_row.agent_name, '') as "agent_name?",
                NULL::text as "context_name?"
            FROM (
                SELECT
                    context_id,
                    MAX(last_activity) AS last_activity,
                    SUM(request_count)::bigint AS ai_requests
                FROM report_ai_context_hourly
                WHERE user_id = $1
                  AND bucket < $2
                  AND traffic = 'production'
                GROUP BY context_id
                ORDER BY MAX(last_activity) DESC NULLS LAST
                LIMIT $3
            ) ctx
            LEFT JOIN LATERAL (
                SELECT model, agent_name FROM report_ai_context_hourly
                WHERE context_id = ctx.context_id
                ORDER BY last_activity DESC NULLS LAST
                LIMIT 1
            ) last_row ON TRUE
            ORDER BY ctx.last_activity DESC
            "#,
            user_id.as_str(),
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        let ids: Vec<String> = rows.iter().map(|row| row.context_id.to_string()).collect();
        let names: HashMap<String, Option<String>> = sqlx::query!(
            "SELECT context_id, name FROM report_user_contexts WHERE context_id = ANY($1)",
            &ids
        )
        .fetch_all(&*self.pool)
        .await?
        .into_iter()
        .filter_map(|row| Some((row.context_id?, row.name)))
        .collect();
        for row in &mut rows {
            row.context_name = names.get(row.context_id.as_str()).cloned().flatten();
        }
        Ok(rows.into_iter().map(RecentContextRow::from).collect())
    }
}
