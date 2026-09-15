//! Lifecycle helpers for the streaming event loop:
//! - emit the A2A `working` status update when streaming begins,
//! - the single emitter for A2A `TaskStatusUpdate` frames on the SSE channel
//!   (exactly one frame per task carries `final: true`),
//! - record stream-creation failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::AgentServiceError;
use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder};
use tokio::sync::mpsc::Sender;

use crate::models::a2a::protocol::TaskStatusUpdateEvent;
use crate::models::a2a::{TaskState, TaskStatus};
use crate::repository::task::TaskRepository;

use super::webhook_client::WebhookContext;

pub(super) async fn send_a2a_status_event(
    tx: &Sender<Event>,
    task_id: &TaskId,
    context_id: &ContextId,
    status: TaskStatus,
    is_final: bool,
) {
    let event = TaskStatusUpdateEvent::new(task_id.clone(), context_id.clone(), status, is_final);
    let jsonrpc = event.to_jsonrpc_response();
    if tx
        .send(Event::default().data(jsonrpc.to_string()))
        .await
        .is_err()
    {
        tracing::trace!("Failed to send status event, channel closed");
    }
}

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct EmitRunStartedParams<'a> {
    pub tx: &'a Sender<Event>,
    pub webhook_context: &'a WebhookContext,
    pub context_id: &'a ContextId,
    pub task_id: &'a TaskId,
    pub task_repo: &'a TaskRepository,
}

pub async fn emit_run_started(params: EmitRunStartedParams<'_>) {
    let EmitRunStartedParams {
        tx,
        webhook_context,
        context_id,
        task_id,
        task_repo,
    } = params;
    let working_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_state(task_id, TaskState::Working, &working_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task state");
        return;
    }

    send_a2a_status_event(
        tx,
        task_id,
        context_id,
        TaskStatus {
            state: TaskState::Working,
            message: None,
            timestamp: Some(working_timestamp),
        },
        false,
    )
    .await;

    let a2a_event = A2AEventBuilder::task_status_update(
        task_id.clone(),
        context_id.clone(),
        TaskState::Working,
        None,
    );
    if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
        tracing::error!(error = %e, "Failed to broadcast A2A working");
    }

    let event = AgUiEventBuilder::run_started(context_id.clone(), task_id.clone(), None);

    if let Err(e) = webhook_context.broadcast_agui(event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_STARTED");
    }
}

pub async fn handle_stream_creation_error(
    webhook_context: &WebhookContext,
    error: AgentServiceError,
    task_id: &TaskId,
    _context_id: &ContextId,
    task_repo: &TaskRepository,
) {
    let error_msg = format!("Failed to create stream: {error}");
    tracing::error!(task_id = %task_id, error = %error, "Failed to create stream");

    let failed_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_failed_with_error(task_id, &error_msg, &failed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task to failed state");
    }

    let error_event = AgUiEventBuilder::run_error(
        format!("Failed to process message: {error}"),
        Some("STREAM_CREATION_ERROR".to_owned()),
    );
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}
