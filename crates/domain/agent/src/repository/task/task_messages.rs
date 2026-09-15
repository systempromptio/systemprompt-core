//! Task message-history persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TaskRepository;
use crate::models::a2a::{Message, Part};
use crate::repository::context::message::{
    get_message_parts, get_messages_by_context, get_messages_by_task, message_exists,
};
use systemprompt_traits::RepositoryError;

impl TaskRepository {
    pub async fn message_exists(
        &self,
        message_id: &systemprompt_identifiers::MessageId,
    ) -> Result<bool, RepositoryError> {
        message_exists(&self.pool, message_id).await
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
}
