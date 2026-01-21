mod parts;
mod persistence;
mod queries;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Message;

pub use parts::{get_message_parts, FileUploadContext};
pub use persistence::{persist_message_sqlx, persist_message_with_tx};
pub use queries::{
    get_messages_by_context, get_messages_by_task, get_next_sequence_number,
    get_next_sequence_number_in_tx, get_next_sequence_number_sqlx,
};

#[derive(Debug, Clone)]
pub struct MessageRepository {
    pool: Arc<PgPool>,
}

impl MessageRepository {
    pub fn new(db_pool: DbPool) -> Result<Self, RepositoryError> {
        let pool = db_pool.as_ref().get_postgres_pool().ok_or_else(|| {
            RepositoryError::InvalidData("PostgreSQL pool not available".to_string())
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
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        message: &Message,
        task_id: &TaskId,
        context_id: &ContextId,
        sequence_number: i32,
        user_id: Option<&UserId>,
        session_id: &SessionId,
        trace_id: &TraceId,
        upload_ctx: Option<&FileUploadContext<'_>>,
    ) -> Result<(), RepositoryError> {
        persist_message_sqlx(
            tx,
            message,
            task_id,
            context_id,
            sequence_number,
            user_id,
            session_id,
            trace_id,
            upload_ctx,
        )
        .await
    }

    pub async fn persist_message_with_tx(
        &self,
        tx: &mut dyn systemprompt_database::DatabaseTransaction,
        message: &Message,
        task_id: &TaskId,
        context_id: &ContextId,
        sequence_number: i32,
        user_id: Option<&UserId>,
        session_id: &SessionId,
        trace_id: &TraceId,
    ) -> Result<(), RepositoryError> {
        persist_message_with_tx(
            tx,
            message,
            task_id,
            context_id,
            sequence_number,
            user_id,
            session_id,
            trace_id,
        )
        .await
    }
}
