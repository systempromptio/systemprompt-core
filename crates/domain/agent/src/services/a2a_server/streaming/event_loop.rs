//! Streaming event loop — fans `StreamEvent`s from the AI model out to A2A
//! status frames, AG-UI webhooks, the SSE channel, and the task repository.
//! Exactly one terminal status frame (`final: true`) is emitted per task,
//! whichever way the pipeline ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, TaskId};
use systemprompt_models::{AgUiEventBuilder, CallToolResult, RequestContext, ToolCall};
use tokio::sync::mpsc::Sender;

use crate::models::ExecutionStep;
use crate::models::a2a::{Artifact, Message, TaskState};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::processing::message::{
    MessageProcessor, MessageStream, StreamEvent,
};

use super::event_loop_lifecycle::{EmitRunStartedParams, emit_run_started};
use super::handlers::{
    AnnounceFailureParams, HandleCompleteParams, TextStreamState, announce_cancelled,
    announce_failure, handle_complete, record_failure,
};
use super::webhook_client::WebhookContext;

pub struct ProcessEventsParams {
    pub tx: Sender<Event>,
    pub stream: MessageStream,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: MessageId,
    pub original_message: Message,
    pub agent_name: String,
    pub context: RequestContext,
    pub task_repo: TaskRepository,
    pub processor: Arc<MessageProcessor>,
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

pub async fn process_events(params: ProcessEventsParams) {
    let ProcessEventsParams {
        tx,
        mut stream,
        task_id,
        context_id,
        message_id,
        original_message,
        agent_name,
        context,
        task_repo,
        processor,
    } = params;

    let webhook_context = WebhookContext::for_request(processor.webhooks(), &context);

    emit_run_started(EmitRunStartedParams {
        tx: &tx,
        webhook_context: &webhook_context,
        context_id: &context_id,
        task_id: &task_id,
        task_repo: &task_repo,
    })
    .await;

    tracing::info!("Stream channel received, waiting for events...");

    let mut text_state = TextStreamState::new().with_webhook_context(webhook_context.clone());

    let ctx = EventLoopCtx {
        tx: &tx,
        webhook_context: &webhook_context,
        task_id: &task_id,
        context_id: &context_id,
        message_id: &message_id,
        original_message: &original_message,
        agent_name: &agent_name,
        context: &context,
        task_repo: &task_repo,
        processor: &processor,
    };

    while let Some(event) = stream.events.recv().await {
        match event {
            StreamEvent::Text(text) => text_state.handle_text(text, &message_id).await,
            StreamEvent::ToolCallStarted(tool_call) => {
                broadcast_tool_call_started(&webhook_context, &tool_call, &message_id).await;
            },
            StreamEvent::ToolResult {
                ai_tool_call_id,
                result,
            } => broadcast_tool_result(&webhook_context, &ai_tool_call_id, &result).await,
            StreamEvent::ExecutionStepUpdate { step } => {
                broadcast_execution_step(&webhook_context, step, &context_id).await;
            },
            StreamEvent::Complete {
                full_text,
                artifacts,
            } => {
                text_state.finalize(&message_id).await;
                finish_completed(&ctx, full_text, artifacts).await;
                break;
            },
            StreamEvent::Error(error) => {
                text_state.finalize(&message_id).await;
                record_failure(&task_repo, &task_id, &error).await;
                finish_failed(&ctx, error, "STREAM_ERROR").await;
                break;
            },
            StreamEvent::Cancelled => {
                text_state.finalize(&message_id).await;
                finish_cancelled(&ctx).await;
                break;
            },
        }
    }

    drop(tx);

    tracing::info!("Stream event loop ended");
}

struct EventLoopCtx<'a> {
    tx: &'a Sender<Event>,
    webhook_context: &'a WebhookContext,
    task_id: &'a TaskId,
    context_id: &'a ContextId,
    message_id: &'a MessageId,
    original_message: &'a Message,
    agent_name: &'a str,
    context: &'a RequestContext,
    task_repo: &'a TaskRepository,
    processor: &'a Arc<MessageProcessor>,
}

async fn broadcast_tool_call_started(
    webhook_context: &WebhookContext,
    tool_call: &ToolCall,
    message_id: &MessageId,
) {
    let tool_call_id = tool_call.ai_tool_call_id.as_str();
    let start_event = AgUiEventBuilder::tool_call_start(
        tool_call_id,
        &tool_call.name,
        Some(message_id.to_string()),
    );
    if let Err(e) = webhook_context.broadcast_agui(start_event).await {
        tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_START");
    }

    let args_json = serde_json::to_string(&tool_call.arguments).unwrap_or_else(|_| String::new());
    let args_event = AgUiEventBuilder::tool_call_args(tool_call_id, &args_json);
    if let Err(e) = webhook_context.broadcast_agui(args_event).await {
        tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_ARGS");
    }

    let end_event = AgUiEventBuilder::tool_call_end(tool_call_id);
    if let Err(e) = webhook_context.broadcast_agui(end_event).await {
        tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_END");
    }
}

async fn broadcast_tool_result(
    webhook_context: &WebhookContext,
    ai_tool_call_id: &AiToolCallId,
    result: &CallToolResult,
) {
    let result_value = serde_json::to_value(result).unwrap_or_else(|_| serde_json::Value::Null);
    let result_event = AgUiEventBuilder::tool_call_result(
        uuid::Uuid::new_v4().to_string(),
        ai_tool_call_id.as_str(),
        result_value,
    );
    if let Err(e) = webhook_context.broadcast_agui(result_event).await {
        tracing::error!(error = %e, "Failed to broadcast TOOL_CALL_RESULT");
    }
}

async fn broadcast_execution_step(
    webhook_context: &WebhookContext,
    step: ExecutionStep,
    context_id: &ContextId,
) {
    let step_event = AgUiEventBuilder::execution_step(step, context_id.clone());
    if let Err(e) = webhook_context.broadcast_agui(step_event).await {
        tracing::error!(error = %e, "Failed to broadcast execution_step");
    }
}

async fn finish_completed(ctx: &EventLoopCtx<'_>, full_text: String, artifacts: Vec<Artifact>) {
    let complete_params = HandleCompleteParams {
        tx: ctx.tx,
        webhook_context: ctx.webhook_context,
        full_text,
        artifacts,
        task_id: ctx.task_id,
        context_id: ctx.context_id,
        id: ctx.message_id.as_str(),
        original_message: ctx.original_message,
        agent_name: ctx.agent_name,
        context: ctx.context,
        processor: ctx.processor,
    };
    if let Err(failure) = handle_complete(complete_params).await {
        tracing::error!(task_id = %ctx.task_id, error = %failure.message, "Failed to complete task");
        record_failure(ctx.task_repo, ctx.task_id, &failure.message).await;
        finish_failed(ctx, failure.message, failure.code).await;
    }
}

async fn finish_failed(ctx: &EventLoopCtx<'_>, error: String, code: &str) {
    announce_failure(AnnounceFailureParams {
        tx: ctx.tx,
        webhook_context: ctx.webhook_context,
        error,
        code,
        task_id: ctx.task_id,
        context_id: ctx.context_id,
    })
    .await;
}

async fn finish_cancelled(ctx: &EventLoopCtx<'_>) {
    if let Err(e) = ctx
        .task_repo
        .update_task_state(ctx.task_id, TaskState::Canceled, &chrono::Utc::now())
        .await
    {
        tracing::error!(task_id = %ctx.task_id, error = %e, "Failed to mark task cancelled");
    }
    announce_cancelled(ctx.tx, ctx.webhook_context, ctx.task_id, ctx.context_id).await;
}
