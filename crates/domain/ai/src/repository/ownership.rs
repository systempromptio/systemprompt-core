//! [`OwnerReassignment`] for the ai domain: moves every request, quota
//! bucket and cached thought signature from one user to another in a single
//! transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows};

use crate::error::RepositoryError;

#[derive(Debug, Clone)]
pub struct AiOwnerReassignment {
    write_pool: Arc<PgPool>,
}

impl AiOwnerReassignment {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { write_pool })
    }
}

fn shared_error(e: sqlx::Error) -> systemprompt_traits::RepositoryError {
    systemprompt_traits::RepositoryError::database(e)
}

#[async_trait]
impl OwnerReassignment for AiOwnerReassignment {
    fn domain(&self) -> &'static str {
        "ai"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, systemprompt_traits::RepositoryError> {
        let mut tx = self.write_pool.begin().await.map_err(shared_error)?;

        let requests = sqlx::query!(
            "UPDATE ai_requests SET user_id = $2, updated_at = CURRENT_TIMESTAMP WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(shared_error)?
        .rows_affected();

        // Why: the merged user's buckets are dropped rather than renamed —
        // renaming would collide with the target's own bucket for the same
        // window, and the target's consumption is the surviving record.
        let quota_buckets = sqlx::query!(
            "DELETE FROM ai_quota_buckets WHERE subject_kind = 'user' AND subject_id = $1",
            from.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(shared_error)?
        .rows_affected();

        let thought_signatures = sqlx::query!(
            "UPDATE ai_gateway_thought_signatures SET user_id = $2 WHERE user_id = $1 \
             AND NOT EXISTS (SELECT 1 FROM ai_gateway_thought_signatures t \
             WHERE t.user_id = $2 AND t.conversation_id = ai_gateway_thought_signatures.conversation_id \
             AND t.tool_use_id = ai_gateway_thought_signatures.tool_use_id)",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(shared_error)?
        .rows_affected();

        tx.commit().await.map_err(shared_error)?;

        Ok(ReassignedRows {
            tables: vec![
                ("ai_requests", requests),
                ("ai_quota_buckets", quota_buckets),
                ("ai_gateway_thought_signatures", thought_signatures),
            ],
        })
    }
}
