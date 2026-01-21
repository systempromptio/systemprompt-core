use super::TaskRepository;
use crate::models::a2a::{Message, Part};
use crate::repository::context::message::{
    get_message_parts, get_messages_by_context, get_messages_by_task, get_next_sequence_number,
    get_next_sequence_number_in_tx, persist_message_with_tx,
};
use systemprompt_traits::RepositoryError;

impl TaskRepository {
    pub async fn get_next_sequence_number(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<i32, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_next_sequence_number(&pool, task_id).await
    }

    pub async fn get_messages_by_task(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Vec<Message>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_messages_by_task(&pool, task_id).await
    }

    pub async fn get_message_parts(
        &self,
        message_id: &systemprompt_identifiers::MessageId,
    ) -> Result<Vec<Part>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_message_parts(&pool, message_id).await
    }

    pub async fn get_messages_by_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<Message>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_messages_by_context(&pool, context_id).await
    }

    pub async fn get_next_sequence_number_in_tx(
        &self,
        tx: &mut dyn systemprompt_database::DatabaseTransaction,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<i32, RepositoryError> {
        get_next_sequence_number_in_tx(tx, task_id).await
    }

    pub async fn persist_message_with_tx(
        &self,
        tx: &mut dyn systemprompt_database::DatabaseTransaction,
        message: &Message,
        task_id: &systemprompt_identifiers::TaskId,
        context_id: &systemprompt_identifiers::ContextId,
        sequence_number: i32,
        user_id: Option<&systemprompt_identifiers::UserId>,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
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
