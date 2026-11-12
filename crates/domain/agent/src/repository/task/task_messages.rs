//! Task message-history persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TaskRepository;
use crate::models::a2a::Message;
use crate::repository::OwnedContext;
use crate::repository::context::message::{get_messages_by_context, message_in_owned_context};
use systemprompt_traits::RepositoryError;

impl TaskRepository {
    pub async fn message_in_owned_context(
        &self,
        message_id: &systemprompt_identifiers::MessageId,
        context_id: &systemprompt_identifiers::ContextId,
        user_id: &systemprompt_identifiers::UserId,
    ) -> Result<bool, RepositoryError> {
        message_in_owned_context(&self.pool, message_id, context_id, user_id).await
    }

    pub async fn get_messages_by_context(
        &self,
        owned: &OwnedContext,
    ) -> Result<Vec<Message>, RepositoryError> {
        get_messages_by_context(&self.pool, owned.context_id()).await
    }
}
