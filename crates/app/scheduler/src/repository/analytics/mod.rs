//! Analytics maintenance queries used by scheduled cleanup jobs: empty
//! contexts and the behavioural-bot session flags.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::ContextKind;

use crate::error::SchedulerResult;

#[derive(Debug, Clone)]
pub struct AnalyticsRepository {
    write_pool: Arc<PgPool>,
}

impl AnalyticsRepository {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }

    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> SchedulerResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_contexts
            WHERE context_id IN (
                SELECT uc.context_id
                FROM user_contexts uc
                LEFT JOIN task_messages tm ON uc.context_id = tm.context_id
                WHERE tm.id IS NULL
                AND NOT EXISTS (
                    SELECT 1 FROM mcp_tool_executions mte WHERE mte.context_id = uc.context_id
                )
                AND NOT EXISTS (
                    SELECT 1 FROM governance_decisions gd WHERE gd.context_id = uc.context_id
                )
                AND uc.created_at < NOW() - ($1 || ' hours')::interval
                AND (uc.kind != $2 OR uc.session_id IS NULL)
            )
            "#,
            hours_old.to_string(),
            ContextKind::CliSession.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn count_empty_contexts(&self, hours_old: i64) -> SchedulerResult<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM user_contexts uc
            LEFT JOIN task_messages tm ON uc.context_id = tm.context_id
            WHERE tm.id IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM mcp_tool_executions mte WHERE mte.context_id = uc.context_id
            )
            AND NOT EXISTS (
                SELECT 1 FROM governance_decisions gd WHERE gd.context_id = uc.context_id
            )
            AND uc.created_at < NOW() - ($1 || ' hours')::interval
            AND (uc.kind != $2 OR uc.session_id IS NULL)
            "#,
            hours_old.to_string(),
            ContextKind::CliSession.as_str()
        )
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(count)
    }

    pub async fn mark_ghost_sessions_as_bots(&self) -> SchedulerResult<i64> {
        // Why: a browser heuristic. Bridge, OAuth and API sessions never load
        // a landing page or run JavaScript, so without the source filter every
        // desktop-client session was marked a bot (1,203 of them in production).
        let marked = sqlx::query_scalar!(
            r#"
            WITH cleaned AS (
                UPDATE user_sessions
                SET is_behavioral_bot = true,
                    behavioral_bot_reason = 'ghost_session',
                    behavioral_bot_score = 35
                WHERE is_bot = false
                  AND is_ai_crawler = false
                  AND is_scanner = false
                  AND is_behavioral_bot = false
                  AND session_source = 'web'
                  AND request_count = 0
                  AND landing_page IS NULL
                  AND entry_url IS NULL
                  AND started_at < NOW() - INTERVAL '5 minutes'
                RETURNING 1
            )
            SELECT COUNT(*)::BIGINT as "count!" FROM cleaned
            "#
        )
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(marked)
    }

    pub async fn mark_no_js_sessions_as_bots(&self) -> SchedulerResult<i64> {
        // Why: a browser heuristic. Bridge, OAuth and API sessions never load
        // a landing page or run JavaScript, so without the source filter every
        // desktop-client session was marked a bot (1,203 of them in production).
        let marked = sqlx::query_scalar!(
            r#"
            WITH cleaned AS (
                UPDATE user_sessions
                SET is_behavioral_bot = true,
                    behavioral_bot_reason = 'no_javascript',
                    behavioral_bot_score = 20
                WHERE is_bot = false
                  AND is_ai_crawler = false
                  AND is_scanner = false
                  AND is_behavioral_bot = false
                  AND session_source = 'web'
                  AND request_count > 0
                  AND started_at < NOW() - INTERVAL '10 minutes'
                  AND session_id NOT IN (
                    SELECT DISTINCT session_id FROM engagement_events
                    WHERE time_on_page_ms > 0
                  )
                RETURNING 1
            )
            SELECT COUNT(*)::BIGINT as "count!" FROM cleaned
            "#
        )
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(marked)
    }
}
