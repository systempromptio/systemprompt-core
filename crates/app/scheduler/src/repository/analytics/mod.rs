//! Analytics maintenance queries used by scheduled cleanup jobs: empty
//! contexts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_models::ContextKind;

use crate::error::SchedulerResult;

#[derive(Debug, Clone)]
pub struct AnalyticsRepository {
    write_pool: Arc<PgPool>,
    contexts: ContextRepository,
}

impl AnalyticsRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
            contexts: ContextRepository::new(db),
        }
    }

    // Why: the candidate set spans four owners (contexts, messages, tool
    // executions, decisions) and is only read here; the delete runs in the
    // agent repository, which owns `user_contexts`.
    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> SchedulerResult<u64> {
        let candidates = self.empty_context_ids(hours_old).await?;
        let deleted = self
            .contexts
            .delete_empty_contexts(&candidates, hours_old, ContextKind::CliSession)
            .await?;
        Ok(deleted)
    }

    async fn empty_context_ids(&self, hours_old: i64) -> SchedulerResult<Vec<ContextId>> {
        let ids = sqlx::query_scalar!(
            r#"
            SELECT uc.context_id as "context_id!"
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
        .fetch_all(&*self.write_pool)
        .await?;
        // Why: a context row whose id is not a valid ContextId cannot be
        // addressed through the agent repository; it is reported and left.
        Ok(ids
            .into_iter()
            .filter_map(|raw| match ContextId::try_new(raw) {
                Ok(id) => Some(id),
                Err(error) => {
                    tracing::warn!(error = %error, "empty-context sweep skipped a context with an invalid id");
                    None
                },
            })
            .collect())
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
