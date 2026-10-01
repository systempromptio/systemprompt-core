//! [`OwnerReassignment`] for the agent domain: moves every context, task and
//! message row from one user to another inside a single transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows, RepositoryError};

#[derive(Debug, Clone)]
pub struct AgentOwnerReassignment {
    write_pool: Arc<PgPool>,
}

impl AgentOwnerReassignment {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }
}

#[async_trait]
impl OwnerReassignment for AgentOwnerReassignment {
    fn domain(&self) -> &'static str {
        "agent"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError> {
        let mut tx = self.write_pool.begin().await?;

        let contexts = sqlx::query!(
            "UPDATE user_contexts SET user_id = $2, updated_at = CURRENT_TIMESTAMP WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        let tasks = sqlx::query!(
            "UPDATE agent_tasks SET user_id = $2, updated_at = CURRENT_TIMESTAMP WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        let messages = sqlx::query!(
            "UPDATE task_messages SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        Ok(ReassignedRows {
            tables: vec![
                ("user_contexts", contexts),
                ("agent_tasks", tasks),
                ("task_messages", messages),
            ],
        })
    }
}
