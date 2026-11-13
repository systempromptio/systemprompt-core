//! The empty-context sweep behind the `cleanup_empty_contexts` job: contexts
//! with no task message, tool execution or governance decision, older than
//! the window. The candidate set spans four owners and is only read here; the
//! delete runs in [`ContextRepository`], which owns `user_contexts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_traits::RepositoryError;

use super::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_models::ContextKind;

#[derive(Debug, Clone)]
pub struct EmptyContextRepository {
    write_pool: Arc<PgPool>,
    contexts: ContextRepository,
}

impl EmptyContextRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
            contexts: ContextRepository::new(db),
        }
    }

    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> Result<u64, RepositoryError> {
        let candidates = self.empty_context_ids(hours_old).await?;
        let deleted = self
            .contexts
            .delete_empty_contexts(&candidates, hours_old, ContextKind::CliSession)
            .await?;
        Ok(deleted)
    }

    async fn empty_context_ids(&self, hours_old: i64) -> Result<Vec<ContextId>, RepositoryError> {
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

    pub async fn count_empty_contexts(&self, hours_old: i64) -> Result<i64, RepositoryError> {
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
