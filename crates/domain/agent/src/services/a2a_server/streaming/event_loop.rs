//! Streaming event loop — fans `StreamEvent`s from the AI model out to A2A
//! status frames, AG-UI webhooks, the SSE channel, and the task repository.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder, RequestContext};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{Message, TaskState};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::processing::message::{MessageProcessor, StreamEvent};

use super::event_loop_lifecycle::{
    EmitRunStartedParams, SendA2aStatusEventParams, emit_run_started, send_a2a_status_event,
};
use super::handlers::text::TextStreamState;
use super::handlers::{HandleCompleteParams, HandleErrorParams, handle_complete, handle_error};
use super::webhook_client::WebhookContext;

/// Parameters for [`process_events`].
pub struct ProcessEventsParams {
    pub tx: Sender<Event>,
    pub chunk_rx: Receiver<StreamEvent>,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: MessageId,
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

/// Drive the streaming event loop until the model's `Complete`/`Error` event
/// arrives.
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
        WebhookContext::new(context.user_id().clone(), context.auth_token().as_str());

    emit_run_started(EmitRunStartedParams {
        tx: &tx,
        webhook_context: &webhook_context,
        context_id: &context_id,
        task_id: &task_id,
        task_repo: &task_repo,
        request_id: &request_id,
    })
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
                    Some(message_id.to_string()),
                );
                if let Err(e) = webhook_context.broadcast_agui(start_event).await {
                    tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_START");
                }

                let args_json =
                    serde_json::to_string(&tool_call.arguments).unwrap_or_else(|_| String::new());
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
                let result_value =
                    serde_json::to_value(&result).unwrap_or_else(|_| serde_json::Value::Null);
                let result_event = AgUiEventBuilder::tool_call_result(
                    uuid::Uuid::new_v4().to_string(),
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
                    id: message_id.as_str(),
                    original_message: &original_message,
                    agent_name: &agent_name,
                    context: &context,
                    auth_token: context.auth_token().as_str(),
                    task_repo: &task_repo,
                    processor: &processor,
                };
                handle_complete(complete_params).await;

                send_a2a_status_event(&SendA2aStatusEventParams {
                    tx: &tx,
                    task_id: &task_id,
                    context_id: &context_id,
                    state: "completed",
                    is_final: true,
                    request_id: &request_id,
                });

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
                handle_error(HandleErrorParams {
                    tx: &tx,
                    webhook_context: &webhook_context,
                    error,
                    task_id: &task_id,
                    context_id: &context_id,
                    task_repo: &task_repo,
                })
                .await;

                send_a2a_status_event(&SendA2aStatusEventParams {
                    tx: &tx,
                    task_id: &task_id,
                    context_id: &context_id,
                    state: "failed",
                    is_final: true,
                    request_id: &request_id,
                });

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
