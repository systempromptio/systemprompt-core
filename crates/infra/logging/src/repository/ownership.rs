//! [`OwnerReassignment`] for the logging tables: moves every log line and
//! analytics event from one user to another in a single transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows, RepositoryError};

use crate::models::LoggingError;

#[derive(Debug, Clone)]
pub struct LoggingOwnerReassignment {
    write_pool: Arc<PgPool>,
}

impl LoggingOwnerReassignment {
    pub fn new(db: &DbPool) -> Result<Self, LoggingError> {
        let write_pool = db.write_pool_arc()?;
        Ok(Self { write_pool })
    }
}

#[async_trait]
impl OwnerReassignment for LoggingOwnerReassignment {
    fn domain(&self) -> &'static str {
        "logging"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError> {
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;

        let logs = sqlx::query!(
            "UPDATE logs SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        let events = sqlx::query!(
            "UPDATE analytics_events SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        tx.commit().await.map_err(RepositoryError::database)?;

        Ok(ReassignedRows {
            tables: vec![("logs", logs), ("analytics_events", events)],
        })
    }
}
