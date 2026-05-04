//! Lifecycle helpers for the streaming event loop:
//! - emit the A2A `working` status update when streaming begins,
//! - emit a status frame on the SSE channel,
//! - record stream-creation failures.

use axum::response::sse::Event;
use serde_json::json;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder};
use tokio::sync::mpsc::Sender;

use crate::models::a2a::TaskState;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::repository::task::TaskRepository;

use super::webhook_client::WebhookContext;

pub(super) struct SendA2aStatusEventParams<'a> {
    pub tx: &'a Sender<Event>,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub state: &'a str,
    pub is_final: bool,
    pub request_id: &'a NumberOrString,
}

pub(super) fn send_a2a_status_event(params: &SendA2aStatusEventParams<'_>) {
    let SendA2aStatusEventParams {
        tx,
        task_id,
        context_id,
        state,
        is_final,
        request_id,
    } = params;
    let event = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {
            "kind": "status-update",
            "taskId": task_id.as_str(),
            "contextId": context_id.as_str(),
            "status": {
                "state": state,
                "timestamp": chrono::Utc::now().to_rfc3339()
            },
            "final": is_final
        }
    });
    if tx
        .try_send(Event::default().data(event.to_string()))
        .is_err()
    {
        tracing::trace!("Failed to send status event, channel closed");
    }
}

#[allow(missing_debug_implementations)]
pub struct EmitRunStartedParams<'a> {
    pub tx: &'a Sender<Event>,
    pub webhook_context: &'a WebhookContext,
    pub context_id: &'a ContextId,
    pub task_id: &'a TaskId,
    pub task_repo: &'a TaskRepository,
    pub request_id: &'a NumberOrString,
}

pub async fn emit_run_started(params: EmitRunStartedParams<'_>) {
    let EmitRunStartedParams {
        tx,
        webhook_context,
        context_id,
        task_id,
        task_repo,
        request_id,
    } = params;
    let working_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_state(task_id, TaskState::Working, &working_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task state");
        return;
    }

    send_a2a_status_event(&SendA2aStatusEventParams {
        tx,
        task_id,
        context_id,
        state: "working",
        is_final: false,
        request_id,
    });

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
    error: anyhow::Error,
    task_id: &TaskId,
    _context_id: &ContextId,
    task_repo: &TaskRepository,
) {
    let error_msg = format!("Failed to create stream: {}", error);
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
        Some("STREAM_CREATION_ERROR".to_string()),
    );
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}
