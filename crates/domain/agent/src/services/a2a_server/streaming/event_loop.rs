use std::sync::Arc;

use axum::response::sse::Event;
use serde_json::json;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder, RequestContext};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{Message, TaskState};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::processing::message::{MessageProcessor, StreamEvent};

use super::handlers::text::TextStreamState;
use super::handlers::{handle_complete, handle_error, HandleCompleteParams};
use super::webhook_client::WebhookContext;

pub struct ProcessEventsParams {
    pub tx: UnboundedSender<Event>,
    pub chunk_rx: UnboundedReceiver<StreamEvent>,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: String,
    pub original_message: Message,
    pub agent_name: String,
    pub context: RequestContext,
    pub task_repo: TaskRepository,
    pub processor: Arc<MessageProcessor>,
    pub request_id: NumberOrString,
}

impl std::fmt::Debug for ProcessEventsParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessEventsParams")
            .field("task_id", &self.task_id)
            .field("context_id", &self.context_id)
            .field("message_id", &self.message_id)
            .field("agent_name", &self.agent_name)
            .finish_non_exhaustive()
    }
}

fn send_a2a_status_event(
    tx: &UnboundedSender<Event>,
    task_id: &TaskId,
    context_id: &ContextId,
    state: &str,
    is_final: bool,
    request_id: &NumberOrString,
) {
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
    let _ = tx.send(Event::default().data(event.to_string()));
}

pub async fn emit_run_started(
    tx: &UnboundedSender<Event>,
    webhook_context: &WebhookContext,
    context_id: &ContextId,
    task_id: &TaskId,
    task_repo: &TaskRepository,
    request_id: &NumberOrString,
) {
    let working_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_state(&task_id, TaskState::Working, &working_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task state");
        return;
    }

    send_a2a_status_event(tx, task_id, context_id, "working", false, request_id);

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

pub async fn process_events(params: ProcessEventsParams) {
    let ProcessEventsParams {
        tx,
        mut chunk_rx,
        task_id,
        context_id,
        message_id,
        original_message,
        agent_name,
        context,
        task_repo,
        processor,
        request_id,
    } = params;

    let webhook_context =
        WebhookContext::new(context.user_id().as_str(), context.auth_token().as_str());

    emit_run_started(
        &tx,
        &webhook_context,
        &context_id,
        &task_id,
        &task_repo,
        &request_id,
    )
    .await;

    tracing::info!("Stream channel received, waiting for events...");

    let mut text_state = TextStreamState::new().with_webhook_context(webhook_context.clone());

    while let Some(event) = chunk_rx.recv().await {
        match event {
            StreamEvent::Text(text) => {
                text_state.handle_text(text, &message_id).await;
            },
            StreamEvent::ToolCallStarted(tool_call) => {
                let tool_call_id = tool_call.ai_tool_call_id.as_str();
                let start_event = AgUiEventBuilder::tool_call_start(
                    tool_call_id,
                    &tool_call.name,
                    Some(message_id.clone()),
                );
                if let Err(e) = webhook_context.broadcast_agui(start_event).await {
                    tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_START");
                }

                let args_json = serde_json::to_string(&tool_call.arguments).unwrap_or_default();
                let args_event = AgUiEventBuilder::tool_call_args(tool_call_id, &args_json);
                if let Err(e) = webhook_context.broadcast_agui(args_event).await {
                    tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_ARGS");
                }

                let end_event = AgUiEventBuilder::tool_call_end(tool_call_id);
                if let Err(e) = webhook_context.broadcast_agui(end_event).await {
                    tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_END");
                }
            },
            StreamEvent::ToolResult { call_id, result } => {
                let result_value = serde_json::to_value(&result).unwrap_or_default();
                let result_event = AgUiEventBuilder::tool_call_result(
                    &uuid::Uuid::new_v4().to_string(),
                    &call_id,
                    result_value,
                );
                if let Err(e) = webhook_context.broadcast_agui(result_event).await {
                    tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_RESULT");
                }
            },
            StreamEvent::ExecutionStepUpdate { step } => {
                let step_event = AgUiEventBuilder::execution_step(step.clone(), context_id.clone());
                if let Err(e) = webhook_context.broadcast_agui(step_event).await {
                    tracing::error!(error = %e, "Failed to broadcast execution_step");
                }
            },
            StreamEvent::Complete {
                full_text,
                artifacts,
            } => {
                text_state.finalize(&message_id).await;

                let complete_params = HandleCompleteParams {
                    tx: &tx,
                    webhook_context: &webhook_context,
                    full_text,
                    artifacts,
                    task_id: &task_id,
                    context_id: &context_id,
                    id: &message_id,
                    original_message: &original_message,
                    agent_name: &agent_name,
                    context: &context,
                    auth_token: context.auth_token().as_str(),
                    task_repo: &task_repo,
                    processor: &processor,
                };
                handle_complete(complete_params).await;

                send_a2a_status_event(&tx, &task_id, &context_id, "completed", true, &request_id);

                let a2a_event = A2AEventBuilder::task_status_update(
                    task_id.clone(),
                    context_id.clone(),
                    TaskState::Completed,
                    None,
                );
                if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
                    tracing::error!(error = %e, "Failed to broadcast A2A completed");
                }

                break;
            },
            StreamEvent::Error(error) => {
                text_state.finalize(&message_id).await;
                handle_error(
                    &tx,
                    &webhook_context,
                    error,
                    &task_id,
                    &context_id,
                    &task_repo,
                )
                .await;

                send_a2a_status_event(&tx, &task_id, &context_id, "failed", true, &request_id);

                let a2a_event = A2AEventBuilder::task_status_update(
                    task_id.clone(),
                    context_id.clone(),
                    TaskState::Failed,
                    None,
                );
                if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
                    tracing::error!(error = %e, "Failed to broadcast A2A failed");
                }

                break;
            },
        }
    }

    drop(tx);

    tracing::info!("Stream event loop ended");
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
    let _ = task_repo
        .update_task_failed_with_error(task_id, &error_msg, &failed_timestamp)
        .await;

    let error_event = AgUiEventBuilder::run_error(
        format!("Failed to process message: {error}"),
        Some("STREAM_CREATION_ERROR".to_string()),
    );
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}
