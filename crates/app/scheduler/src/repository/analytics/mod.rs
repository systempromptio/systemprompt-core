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
        // Why: a CLI-session context bound to a live session must survive even
        // when empty — deleting it would force the CLI to re-mint a context on
        // its next run. It becomes collectable once its session is gone
        // (the FK sets session_id NULL on session deletion).
        let result = sqlx::query!(
            r#"
            DELETE FROM user_contexts
            WHERE context_id IN (
                SELECT uc.context_id
                FROM user_contexts uc
                LEFT JOIN task_messages tm ON uc.context_id = tm.context_id
                WHERE tm.id IS NULL
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
}
