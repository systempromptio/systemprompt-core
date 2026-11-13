//! The empty-context sweep behind the `cleanup_empty_contexts` job: contexts
//! with no task message, tool execution or governance decision, older than
//! the window.
//!
//! The candidates (no message, old enough, not a session-bound CLI context)
//! come from this crate's own tables. Contexts an audit row still references
//! are then removed from the set through their owners: MCP tool executions
//! through the [`DynToolExecutionLookup`] the mcp extension provides, and
//! governance decisions through the governance crate. The delete runs in
//! [`ContextRepository`], which owns `user_contexts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_governance::GovernanceDecisionRetention;
use systemprompt_identifiers::ContextId;
use systemprompt_models::ContextKind;
use systemprompt_traits::{DynToolExecutionLookup, RepositoryError};

use super::ContextRepository;

/// Finds and deletes contexts nothing references.
#[derive(Clone)]
pub struct EmptyContextRepository {
    write_pool: Arc<PgPool>,
    contexts: ContextRepository,
    tool_executions: DynToolExecutionLookup,
    decisions: GovernanceDecisionRetention,
}

impl std::fmt::Debug for EmptyContextRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmptyContextRepository")
            .finish_non_exhaustive()
    }
}

impl EmptyContextRepository {
    pub fn new(db: &DbPool, tool_executions: DynToolExecutionLookup) -> Self {
        Self {
            write_pool: db.write_pool(),
            contexts: ContextRepository::new(db),
            tool_executions,
            decisions: GovernanceDecisionRetention::new(db),
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

    pub async fn count_empty_contexts(&self, hours_old: i64) -> Result<i64, RepositoryError> {
        let candidates = self.empty_context_ids(hours_old).await?;
        Ok(i64::try_from(candidates.len()).unwrap_or(i64::MAX))
    }

    async fn empty_context_ids(&self, hours_old: i64) -> Result<Vec<ContextId>, RepositoryError> {
        let candidates = self.unreferenced_by_messages(hours_old).await?;
        let executed = self
            .tool_executions
            .contexts_with_executions(&candidates)
            .await?;
        let decided = self.decisions.contexts_with_decisions(&candidates).await?;
        Ok(candidates
            .into_iter()
            .filter(|id| !executed.contains(id) && !decided.contains(id))
            .collect())
    }

    async fn unreferenced_by_messages(
        &self,
        hours_old: i64,
    ) -> Result<Vec<ContextId>, RepositoryError> {
        let ids = sqlx::query_scalar!(
            r#"
            SELECT uc.context_id as "context_id!"
            FROM user_contexts uc
            LEFT JOIN task_messages tm ON uc.context_id = tm.context_id
            WHERE tm.id IS NULL
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
}
