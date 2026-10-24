//! Non-streaming A2A message handling.
//!
//! `SendMessage` answers with the finished task (`SendMessageResult::Task`).
//! `returnImmediately` is accepted but not honoured: the message pipeline
//! persists and completes the task in one call, so the reply always waits for
//! the terminal state.
//! `GetTask`, `CancelTask` and `ListTasks` are bound to the caller: a task
//! owned by another user is answered as not found and never listed.
//! `CancelTask` stops the running pipeline on this server and persists the
//! `Canceled` state; a task backed by an agent run is cancelled through the
//! run (see [`super::agent_run`]). `GetExtendedAgentCard` reports that no
//! extended card is configured, and `SubscribeToTask` is an unsupported
//! operation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{AgentName, TaskId};
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::methods;
use systemprompt_traits::BoxedSource;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::SendMessageParams;
use crate::models::a2a::{A2aRequestParams, A2aResponse, SendMessageResult, Task, TaskState};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::errors::{JsonRpcErrorBuilder, JsonRpcErrorResponse};
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageHandler;
use crate::services::shared::AgentServiceError;

use super::list_tasks::{ListTasksError, list_caller_tasks, trim_history};
use super::validation::{ContextValidationError, validate_message_context, validate_task_owner};

const CANCEL_SETTLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Debug)]
pub(super) enum RequestFailure {
    InvalidParams(String),
    TaskNotFound(TaskId),
    TaskNotCancelable(TaskId),
    Unsupported(&'static str),
    ExtendedAgentCardNotConfigured,
    Internal {
        context: &'static str,
        source: BoxedSource,
    },
}

impl RequestFailure {
    pub(super) fn internal<E>(context: &'static str, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Internal {
            context,
            source: Box::new(source),
        }
    }

    pub(super) fn into_jsonrpc(self, request_id: &NumberOrString) -> JsonRpcErrorResponse {
        match self {
            Self::InvalidParams(message) => JsonRpcErrorBuilder::invalid_params()
                .with_metadata("detail", message)
                .log_warn("A2A request rejected: invalid params")
                .build(request_id),
            Self::TaskNotFound(task_id) => JsonRpcErrorBuilder::task_not_found(&task_id)
                .log_warn(format!("A2A task not found: {task_id}"))
                .build(request_id),
            Self::TaskNotCancelable(task_id) => JsonRpcErrorBuilder::task_not_cancelable(&task_id)
                .log_warn(format!("A2A task not cancelable: {task_id}"))
                .build(request_id),
            Self::Unsupported(operation) => JsonRpcErrorBuilder::unsupported_operation()
                .with_metadata("method", operation)
                .log_warn(format!("Unsupported A2A operation: {operation}"))
                .build(request_id),
            Self::ExtendedAgentCardNotConfigured => {
                JsonRpcErrorBuilder::extended_agent_card_not_configured()
                    .log_warn("Extended agent card requested but not configured")
                    .build(request_id)
            },
            Self::Internal { context, source } => JsonRpcErrorBuilder::internal_error()
                .with_metadata("detail", context)
                .log_error(format!("A2A request handling failed: {context}: {source}"))
                .build(request_id),
        }
    }
}

impl From<ContextValidationError> for RequestFailure {
    fn from(error: ContextValidationError) -> Self {
        match error {
            ContextValidationError::TaskNotFound(task_id) => Self::TaskNotFound(task_id),
            ContextValidationError::TaskLookup(source) => {
                Self::internal("Task lookup failed", source)
            },
            ContextValidationError::Context(source) if !source.is_not_found() => {
                Self::internal("Context validation failed", source)
            },
            other @ (ContextValidationError::Unauthenticated
            | ContextValidationError::Context(_)) => Self::InvalidParams(other.to_string()),
        }
    }
}

impl From<AgentServiceError> for RequestFailure {
    fn from(error: AgentServiceError) -> Self {
        Self::internal("Request handling failed", error)
    }
}

impl From<ListTasksError> for RequestFailure {
    fn from(error: ListTasksError) -> Self {
        match error {
            ListTasksError::PageToken(source) => {
                Self::InvalidParams(format!("invalid pageToken: {source}"))
            },
            repository @ ListTasksError::Repository(_) => {
                Self::internal("Task listing failed", repository)
            },
        }
    }
}

pub(super) async fn handle_non_streaming_request(
    request: A2aRequestParams,
    state: &AgentHandlerState,
    context: &RequestContext,
    request_id: &NumberOrString,
) -> Result<A2aResponse, RequestFailure> {
    let config = state.config.read().await;
    let agent_name = AgentName::new(config.name.clone());
    drop(config);

    let id = request_id.clone();
    match request {
        A2aRequestParams::SendMessage(params) | A2aRequestParams::SendStreamingMessage(params) => {
            tracing::info!("Handling SendMessage request");
            let task = send_message(params, &agent_name, state, context).await?;
            Ok(A2aResponse::send_message(
                SendMessageResult::Task(Box::new(task)),
                id,
            ))
        },
        A2aRequestParams::GetTask(params) => {
            tracing::info!(task_id = %params.id, "Handling GetTask request");
            let task_repo = &state.agent_state.repositories().tasks;
            validate_task_owner(task_repo, &params.id, context.user_id()).await?;
            let mut task = owned_task(task_repo, &params.id).await?;
            trim_history(&mut task, params.history_length);
            Ok(A2aResponse::get_task(task, id))
        },
        A2aRequestParams::ListTasks(params) => {
            tracing::info!("Handling ListTasks request");
            if context.user_id().as_str().is_empty() {
                return Err(ContextValidationError::Unauthenticated.into());
            }
            let task_repo = &state.agent_state.repositories().tasks;
            let result = list_caller_tasks(task_repo, context.user_id(), &params).await?;
            Ok(A2aResponse::list_tasks(result, id))
        },
        A2aRequestParams::CancelTask(params) => {
            tracing::info!(task_id = %params.id, "Handling CancelTask request");
            let task = cancel_task(&params.id, state, context).await?;
            Ok(A2aResponse::cancel_task(task, id))
        },
        A2aRequestParams::GetExtendedAgentCard(_) => {
            Err(RequestFailure::ExtendedAgentCardNotConfigured)
        },
        A2aRequestParams::SubscribeToTask(_) => {
            Err(RequestFailure::Unsupported(methods::SUBSCRIBE_TO_TASK))
        },
    }
}

async fn send_message(
    params: SendMessageParams,
    agent_name: &AgentName,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Task, RequestFailure> {
    let history_length = params
        .configuration
        .as_ref()
        .and_then(|configuration| configuration.history_length);
    let message = params.message;
    validate_message_context(
        &message,
        context.user_id(),
        &state.agent_state.repositories().contexts,
    )
    .await?;

    if let Some(task) =
        super::agent_run::launch_if_configured(&message, agent_name, state, context).await?
    {
        return Ok(task);
    }

    let message_processor = MessageHandler::new(
        Arc::clone(state.agent_state.repositories()),
        Arc::clone(&state.ai_service),
        state.agent_state.webhooks(),
    )?;

    let mut task = message_processor
        .handle_message(message, agent_name, context, &state.active_tasks)
        .await
        .map_err(RequestFailure::from)?;
    trim_history(&mut task, history_length);
    Ok(task)
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

    if super::agent_run::cancel_run(task_id, state).await? {
        return owned_task(task_repo, task_id).await;
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
            .map_err(|e| RequestFailure::internal("Failed to cancel task", e))?;
    }

    owned_task(task_repo, task_id).await
}

async fn owned_task(task_repo: &TaskRepository, task_id: &TaskId) -> Result<Task, RequestFailure> {
    match task_repo.find_task(task_id).await {
        Ok(Some(task)) => Ok(task),
        Ok(None) => Err(RequestFailure::TaskNotFound(task_id.clone())),
        Err(e) => Err(RequestFailure::internal("Failed to retrieve task", e)),
    }
}
