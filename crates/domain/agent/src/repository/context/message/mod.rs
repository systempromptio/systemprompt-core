mod parts;
mod persistence;
mod queries;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Message;

pub use parts::{FileUploadContext, PersistPartSqlxParams, get_message_parts};
pub use persistence::{
    PersistMessageSqlxParams, PersistMessageWithTxParams, persist_message_sqlx,
    persist_message_with_tx,
};
pub use queries::{
    get_messages_by_context, get_messages_by_task, get_next_sequence_number,
    get_next_sequence_number_in_tx, get_next_sequence_number_sqlx,
};

#[derive(Debug, Clone)]
pub struct MessageRepository {
    pool: Arc<PgPool>,
}

impl MessageRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let pool = db.pool_arc().map_err(|e| {
            RepositoryError::InvalidData(format!("PostgreSQL pool not available: {e}"))
        })?;
        Ok(Self { pool })
    }

    pub async fn get_messages_by_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<Message>, RepositoryError> {
        get_messages_by_task(&self.pool, task_id).await
    }

    pub async fn get_messages_by_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Vec<Message>, RepositoryError> {
        get_messages_by_context(&self.pool, context_id).await
    }

    pub async fn get_next_sequence_number(&self, task_id: &TaskId) -> Result<i32, RepositoryError> {
        get_next_sequence_number(&self.pool, task_id).await
    }

    pub async fn persist_message_sqlx(
        &self,
        params: PersistMessageSqlxParams<'_>,
    ) -> Result<(), RepositoryError> {
        persist_message_sqlx(params).await
    }

    pub async fn persist_message_with_tx(
        &self,
        params: PersistMessageWithTxParams<'_>,
    ) -> Result<(), RepositoryError> {
        persist_message_with_tx(params).await
    }
}
