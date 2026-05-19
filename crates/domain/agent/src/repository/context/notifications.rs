//! Persistence for inbound A2A notifications that have not yet been
//! broadcast to subscribed AG-UI clients.
//!
//! One row is inserted per notification received. The `broadcasted` flag
//! flips to `true` once the corresponding fan-out has completed.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentId, ContextId};
use systemprompt_traits::RepositoryError;

#[derive(Debug, Clone)]
pub struct ContextNotificationRepository {
    write_pool: Arc<PgPool>,
}

impl ContextNotificationRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db.write_pool_arc().map_err(|e| {
            RepositoryError::InvalidData(format!("PostgreSQL write pool not available: {e}"))
        })?;
        Ok(Self { write_pool })
    }

    pub async fn insert(
        &self,
        context_id: &ContextId,
        agent_id: &AgentId,
        notification_type: &str,
        notification_data: &serde_json::Value,
    ) -> Result<i32, RepositoryError> {
        let row = sqlx::query!(
            r#"INSERT INTO context_notifications (context_id, agent_id, notification_type, notification_data)
            VALUES ($1, $2, $3, $4)
            RETURNING id"#,
            context_id.as_str(),
            agent_id.as_str(),
            notification_type,
            notification_data,
        )
        .fetch_one(self.write_pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;
        Ok(row.id)
    }

    pub async fn mark_broadcasted(&self, notification_id: i32) -> Result<(), RepositoryError> {
        sqlx::query!(
            "UPDATE context_notifications SET broadcasted = true WHERE id = $1",
            notification_id,
        )
        .execute(self.write_pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;
        Ok(())
    }
}
