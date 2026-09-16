//! A2A request validation: the caller must own the context a message targets
//! and the task it reads or cancels.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::task::TaskRepository;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use systemprompt_identifiers::{TaskId, UserId};

#[derive(Debug, thiserror::Error)]
pub enum ContextValidationError {
    #[error("Authentication required: the request carries no authenticated user")]
    Unauthenticated,
    #[error("Context validation failed: {0}")]
    Context(String),
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),
    #[error("Task lookup failed: {0}")]
    TaskLookup(String),
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
        .map_err(|e| ContextValidationError::Context(e.to_string()))
}

// Why: a task owned by another user is reported as absent rather than
// forbidden so the endpoint does not confirm which task ids exist.
pub async fn validate_task_owner(
    task_repo: &TaskRepository,
    task_id: &TaskId,
    user_id: &UserId,
) -> Result<(), ContextValidationError> {
    if user_id.as_str().is_empty() {
        return Err(ContextValidationError::Unauthenticated);
    }

    let info = task_repo
        .get_task_context_info(task_id)
        .await
        .map_err(|e| ContextValidationError::TaskLookup(e.to_string()))?
        .ok_or_else(|| ContextValidationError::TaskNotFound(task_id.clone()))?;

    match info.user_id {
        Some(owner) if owner == *user_id => Ok(()),
        _ => Err(ContextValidationError::TaskNotFound(task_id.clone())),
    }
}

pub async fn should_require_oauth(state: &AgentHandlerState) -> bool {
    let config = state.config.read().await;
    config.oauth.required
}
