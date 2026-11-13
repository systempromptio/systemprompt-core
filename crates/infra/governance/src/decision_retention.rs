//! Age-based retention over `governance_decisions`: the owner-side purge the
//! scheduler's retention job dispatches to, and the set of contexts a decision
//! still references (such a context is kept by the empty-context sweep).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_models::errors::RepositoryError;

#[derive(Debug, Clone)]
pub struct GovernanceDecisionRetention {
    write_pool: Arc<PgPool>,
}

impl GovernanceDecisionRetention {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    pub async fn purge_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM governance_decisions WHERE ctid = ANY(ARRAY(SELECT ctid FROM governance_decisions WHERE created_at < $1 LIMIT $2))",
            cutoff,
            batch
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn contexts_with_decisions(
        &self,
        contexts: &[ContextId],
    ) -> Result<HashSet<ContextId>, RepositoryError> {
        if contexts.is_empty() {
            return Ok(HashSet::new());
        }
        let keys: Vec<String> = contexts.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query_scalar!(
            r#"SELECT DISTINCT context_id as "context_id!" FROM governance_decisions WHERE context_id = ANY($1)"#,
            &keys
        )
        .fetch_all(&*self.write_pool)
        .await?;
        Ok(rows.into_iter().map(ContextId::new).collect())
    }
}
