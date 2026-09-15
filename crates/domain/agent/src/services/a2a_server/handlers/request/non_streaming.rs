//! Non-streaming A2A message handling.
//!
//! `GetTask` and `CancelTask` are bound to the caller: a task owned by
//! another user is answered as not found. `CancelTask` stops the running
//! pipeline on this server and persists the `Canceled` state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;
use systemprompt_identifiers::TaskId;
use systemprompt_models::RequestContext;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{A2aRequestParams, Task, TaskState};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;
use crate::services::shared::AgentServiceError;

use super::validation::{ContextValidationError, validate_message_context, validate_task_owner};

const CANCEL_SETTLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Debug)]
pub(super) enum RequestFailure {
    InvalidParams(String),
    TaskNotFound(TaskId),
    TaskNotCancelable(TaskId),
    Unsupported,
    Internal(String),
}

impl RequestFailure {
    pub(super) fn into_jsonrpc(self, request_id: &NumberOrString) -> serde_json::Value {
        match self {
            Self::InvalidParams(message) => JsonRpcErrorBuilder::invalid_params()
                .with_data(json!(message))
                .log_warn("A2A request rejected: invalid params")
                .build(request_id),
            Self::TaskNotFound(task_id) => JsonRpcErrorBuilder::new(-32001, "Task not found")
                .with_data(json!(task_id.as_str()))
                .log_warn(format!("A2A task not found: {task_id}"))
                .build(request_id),
            Self::TaskNotCancelable(task_id) => {
                JsonRpcErrorBuilder::new(-32002, "Task cannot be canceled")
                    .with_data(json!(task_id.as_str()))
                    .log_warn(format!("A2A task not cancelable: {task_id}"))
                    .build(request_id)
            },
            Self::Unsupported => JsonRpcErrorBuilder::method_not_found()
                .with_data(json!("Unsupported request type"))
                .log_warn("Unsupported A2A request type")
                .build(request_id),
            Self::Internal(message) => JsonRpcErrorBuilder::internal_error()
                .with_data(json!(format!("Request handling failed: {message}")))
                .log_error(format!("A2A request handling failed: {message}"))
                .build(request_id),
        }
    }
}

impl From<ContextValidationError> for RequestFailure {
    fn from(error: ContextValidationError) -> Self {
        match error {
            ContextValidationError::TaskNotFound(task_id) => Self::TaskNotFound(task_id),
            ContextValidationError::TaskLookup(message) => Self::Internal(message),
            other @ (ContextValidationError::Unauthenticated
            | ContextValidationError::Context(_)) => Self::InvalidParams(other.to_string()),
        }
    }
}

impl From<AgentServiceError> for RequestFailure {
    fn from(error: AgentServiceError) -> Self {
        Self::Internal(error.to_string())
    }
}

pub(super) async fn handle_non_streaming_request(
    request: A2aRequestParams,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Task, RequestFailure> {
    let config = state.config.read().await;
    let agent_name = config.name.clone();
    drop(config);

    match request {
        A2aRequestParams::SendMessage(params) | A2aRequestParams::SendStreamingMessage(params) => {
            tracing::info!("Handling SendMessage request");
            send_message(params.message, &agent_name, state, context).await
        },
        A2aRequestParams::GetTask(params) => {
            tracing::info!(task_id = %params.id, "Handling GetTask request");
            let task_id = TaskId::new(&params.id);
            let task_repo = &state.agent_state.repositories().tasks;
            validate_task_owner(task_repo, &task_id, context.user_id()).await?;
            owned_task(task_repo, &task_id).await
        },
        A2aRequestParams::CancelTask(params) => {
            tracing::info!(task_id = %params.id, "Handling CancelTask request");
            cancel_task(&params.id, state, context).await
        },
        _ => {
            tracing::warn!(request = ?request, "Unsupported A2A request type");
            Err(RequestFailure::Unsupported)
        },
    }
}

async fn send_message(
    message: crate::models::a2a::Message,
    agent_name: &str,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Task, RequestFailure> {
    validate_message_context(
        &message,
        context.user_id(),
        &state.agent_state.repositories().contexts,
    )
    .await?;

    let message_processor = MessageProcessor::new(
        Arc::clone(state.agent_state.repositories()),
        Arc::clone(&state.ai_service),
        state.agent_state.webhooks(),
    )?;

    message_processor
        .handle_message(message, agent_name, context, &state.active_tasks)
        .await
        .map_err(RequestFailure::from)
}

async fn cancel_task(
    task_id: &TaskId,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Task, RequestFailure> {
    let task_repo = &state.agent_state.repositories().tasks;
    validate_task_owner(task_repo, task_id, context.user_id()).await?;

    let task = owned_task(task_repo, task_id).await?;
    if task.status.state.is_terminal() {
        return Err(RequestFailure::TaskNotCancelable(task_id.clone()));
    }

    if state.active_tasks.cancel(task_id) {
        tracing::info!(task_id = %task_id, "Cancellation signalled to the running pipeline");
        if !state
            .active_tasks
            .wait_until_finished(task_id, CANCEL_SETTLE_TIMEOUT)
            .await
        {
            tracing::warn!(task_id = %task_id, "Cancelled pipeline did not settle in time");
        }
    } else {
        task_repo
            .update_task_state(task_id, TaskState::Canceled, &chrono::Utc::now())
            .await
            .map_err(|e| RequestFailure::Internal(format!("Failed to cancel task: {e}")))?;
    }

    owned_task(task_repo, task_id).await
}

async fn owned_task(task_repo: &TaskRepository, task_id: &TaskId) -> Result<Task, RequestFailure> {
    match task_repo.get_task(task_id).await {
        Ok(Some(task)) => Ok(task),
        Ok(None) => Err(RequestFailure::TaskNotFound(task_id.clone())),
        Err(e) => Err(RequestFailure::Internal(format!(
            "Failed to retrieve task: {e}"
        ))),
    }
}
