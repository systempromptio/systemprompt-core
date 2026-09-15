//! Per-step helpers used by [`super::initialization::setup_stream`]:
//! context validation, initial task persistence, and push-notification config
//! storage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_models::TaskMetadata;
use tokio::sync::mpsc::Sender;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{Task, TaskState, TaskStatus};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::handlers::AgentHandlerState;

use super::initialization::create_jsonrpc_error_event;
use super::types::PersistTaskInput;

pub(super) async fn validate_context(
    context_id: &ContextId,
    user_id: &UserId,
    state: &Arc<AgentHandlerState>,
    tx: &Sender<Event>,
    request_id: &NumberOrString,
) -> Result<(), ()> {
    state
        .agent_state
        .repositories()
        .contexts
        .get_context(context_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!(
                context_id = %context_id,
                user_id = %user_id,
                error = %e,
                "Context validation failed"
            );
            if tx
                .try_send(create_jsonrpc_error_event(
                    -32603,
                    &format!("Context validation failed: {e}"),
                    request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
        })?;

    tracing::info!(
        context_id = %context_id,
        user_id = %user_id,
        "Context validated"
    );

    Ok(())
}

pub(super) async fn persist_initial_task(
    input: PersistTaskInput<'_>,
) -> Result<TaskRepository, ()> {
    let PersistTaskInput {
        task_id,
        context_id,
        agent_name,
        context,
        state,
        tx,
        request_id,
    } = input;

    let task_repo = state.agent_state.repositories().tasks.clone();
    let metadata = TaskMetadata::new_agent_message(agent_name.to_owned());

    let task = Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: Some(metadata),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    };

    task_repo
        .create_task(crate::repository::task::RepoCreateTaskParams {
            task: &task,
            user_id: context.user_id(),
            session_id: context.session_id(),
            trace_id: context.trace_id(),
            agent_name,
        })
        .await
        .map_err(|e| {
            tracing::error!(task_id = %task_id, error = %e, "Failed to persist task at start");
            if tx
                .try_send(create_jsonrpc_error_event(
                    -32603,
                    &format!("Failed to create task: {e}"),
                    request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
        })?;

    tracing::info!(task_id = %task_id, "Task persisted to database at stream start");

    if let Err(e) = task_repo
        .track_agent_in_context(context_id, agent_name)
        .await
    {
        tracing::warn!(context_id = %context_id, error = %e, "Failed to track agent in context");
    }

    Ok(task_repo)
}
