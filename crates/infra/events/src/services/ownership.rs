//! [`OwnerReassignment`] for the events tables: moves every `event_outbox`
//! row from one user to another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows, RepositoryError};

#[derive(Debug, Clone)]
pub struct EventsOwnerReassignment {
    pool: PgPool,
}

impl EventsOwnerReassignment {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OwnerReassignment for EventsOwnerReassignment {
    fn domain(&self) -> &'static str {
        "events"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError> {
        let outbox = sqlx::query!(
            "UPDATE event_outbox SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        Ok(ReassignedRows {
            tables: vec![("event_outbox", outbox)],
        })
    }
}
