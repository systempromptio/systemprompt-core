//! Conversation-scoped cost reads for [`AiUsageReportRepository`].
//!
//! Distinct conversation counts per user, model and agent, and the per-user
//! breakdown that carries them. Every aggregate reads the per-conversation
//! hourly rollup (`report_ai_context_hourly`): a distinct count over a window
//! counts the rollup's conversation-hour rows instead of the requests behind
//! them. User names are joined afterwards by id, without an aggregate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::ai::{
    ContextGroupRow, ContextSummaryRow, CostUserBreakdownRow,
};

use super::AiUsageReportRepository;

impl AiUsageReportRepository {
    pub async fn context_summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ContextSummaryRow, RepositoryError> {
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
            user_id as _
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn contexts_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>, RepositoryError> {
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
            user_id as _,
            limit
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn contexts_by_agent_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>, RepositoryError> {
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
            user_id as _,
            limit
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn breakdown_by_user(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostUserBreakdownRow>, RepositoryError> {
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
        .fetch_all(self.pool.as_ref())
        .await?;
        let ids: Vec<String> = rows.iter().map(|row| row.user_id.to_string()).collect();
        let names: HashMap<String, Option<String>> = sqlx::query!(
            "SELECT id, name FROM report_ai_user_names WHERE id = ANY($1)",
            &ids
        )
        .fetch_all(self.pool.as_ref())
        .await?
        .into_iter()
        .filter_map(|row| Some((row.id?, row.name)))
        .collect();
        for row in &mut rows {
            row.name = names.get(row.user_id.as_str()).cloned().flatten();
        }
        Ok(rows)
    }
}
