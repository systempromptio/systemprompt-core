//! Analytics maintenance queries used by scheduled cleanup jobs.
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
    pub fn new(db: &DbPool) -> SchedulerResult<Self> {
        let write_pool = db.write_pool_arc()?;
        Ok(Self { write_pool })
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
}
