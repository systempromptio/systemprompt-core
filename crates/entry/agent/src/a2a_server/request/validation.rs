//! A2A request validation: the caller must own the context a message targets
//! and the task it reads or cancels.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::OwnedTask;
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_traits::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum ContextValidationError {
    #[error("Authentication required: the request carries no authenticated user")]
    Unauthenticated,
    #[error("Context validation failed: {0}")]
    Context(#[source] RepositoryError),
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),
    #[error("Task lookup failed: {0}")]
    TaskLookup(#[source] RepositoryError),
}

pub async fn validate_message_context(
    message: &crate::models::a2a::Message,
    user_id: &UserId,
    context_repo: &crate::repository::ContextRepository,
) -> Result<(), ContextValidationError> {
    if user_id.as_str().is_empty() {
        return Err(ContextValidationError::Unauthenticated);
    }

    context_repo
        .validate_context_ownership(&message.context_id, user_id)
        .await
        .map_err(ContextValidationError::Context)
}

// Why: a task owned by another user is reported as absent rather than
// forbidden so the endpoint does not confirm which task ids exist.
pub async fn owned_task_for(
    task_repo: &TaskRepository,
    task_id: &TaskId,
    user_id: &UserId,
) -> Result<OwnedTask, ContextValidationError> {
    if user_id.as_str().is_empty() {
        return Err(ContextValidationError::Unauthenticated);
    }

    match task_repo.owned_task(task_id, user_id).await {
        Ok(owned) => Ok(owned),
        Err(e) if e.is_not_found() => Err(ContextValidationError::TaskNotFound(task_id.clone())),
        Err(e) => Err(ContextValidationError::TaskLookup(e)),
    }
}

pub async fn should_require_oauth(state: &AgentHandlerState) -> bool {
    let config = state.config.read().await;
    config.oauth.required
}
