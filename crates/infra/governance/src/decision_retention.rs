//! Age-based retention over `governance_decisions`: the owner-side purge the
//! scheduler's retention job dispatches to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
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
}
