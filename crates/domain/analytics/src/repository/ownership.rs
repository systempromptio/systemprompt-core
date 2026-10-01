//! [`OwnerReassignment`] for the analytics domain: moves every engagement
//! event from one user to another and rewrites the source id in each
//! fingerprint's associated-user list, in a single transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows, RepositoryError};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct AnalyticsOwnerReassignment {
    write_pool: Arc<PgPool>,
}

impl AnalyticsOwnerReassignment {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }
}

#[async_trait]
impl OwnerReassignment for AnalyticsOwnerReassignment {
    fn domain(&self) -> &'static str {
        "analytics"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> std::result::Result<ReassignedRows, RepositoryError> {
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;

        let engagement = sqlx::query!(
            "UPDATE engagement_events SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        let fingerprints = sqlx::query!(
            "UPDATE fingerprint_reputation SET associated_user_ids = \
             array_replace(associated_user_ids, $1, $2) WHERE $1 = ANY(associated_user_ids)",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        tx.commit().await.map_err(RepositoryError::database)?;

        Ok(ReassignedRows {
            tables: vec![
                ("engagement_events", engagement),
                ("fingerprint_reputation", fingerprints),
            ],
        })
    }
}
