use super::TaskRepository;
use crate::models::a2a::{Message, Part};
use crate::repository::context::message::{
    PersistMessageWithTxParams, get_message_parts, get_messages_by_context, get_messages_by_task,
    get_next_sequence_number, get_next_sequence_number_in_tx, persist_message_with_tx,
};
use systemprompt_traits::RepositoryError;

impl TaskRepository {
    pub async fn get_next_sequence_number(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<i32, RepositoryError> {
        get_next_sequence_number(&self.pool, task_id).await
    }

    pub async fn get_messages_by_task(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Vec<Message>, RepositoryError> {
        get_messages_by_task(&self.pool, task_id).await
    }

    pub async fn get_message_parts(
        &self,
        message_id: &systemprompt_identifiers::MessageId,
    ) -> Result<Vec<Part>, RepositoryError> {
        get_message_parts(&self.pool, message_id).await
    }

    pub async fn get_messages_by_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<Message>, RepositoryError> {
        get_messages_by_context(&self.pool, context_id).await
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
        params: PersistMessageWithTxParams<'_>,
    ) -> Result<(), RepositoryError> {
        persist_message_with_tx(params).await
    }
}
