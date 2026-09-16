//! Message-handler helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::{AgUiEventBuilder, AgUiMessageRole};

use crate::models::a2a::{Artifact, Message, Task};
use crate::services::a2a_server::processing::message::{MessageStream, StreamEvent};
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;
use crate::services::shared::{AgentServiceError, Result};

pub(super) async fn collect_stream_response(
    mut stream: MessageStream,
    webhooks: &WebhookContext,
) -> Result<(String, Vec<Artifact>)> {
    let mut response_text = String::new();
    let mut tool_artifacts = Vec::new();

    while let Some(event) = stream.events.recv().await {
        match event {
            StreamEvent::Text(text) => {
                response_text.push_str(&text);
            },
            StreamEvent::Complete {
                full_text,
                artifacts,
            } => {
                response_text = full_text;
                tool_artifacts = artifacts;
            },
            StreamEvent::Error(error) => {
                let error_event =
                    AgUiEventBuilder::run_error(error.clone(), Some("EXECUTION_ERROR".to_owned()));
                if let Err(e) = webhooks.broadcast_agui(error_event).await {
                    tracing::debug!(error = %e, "Failed to broadcast error event");
                }
                return Err(AgentServiceError::Internal(error));
            },
            StreamEvent::Cancelled => return Err(AgentServiceError::TaskCancelled),
            StreamEvent::ToolCallStarted(_)
            | StreamEvent::ToolResult { .. }
            | StreamEvent::ExecutionStepUpdate { .. } => {},
        }
    }

    Ok((response_text, tool_artifacts))
}

pub(super) struct BroadcastAguiLifecycleParams<'a> {
    pub webhooks: &'a WebhookContext,
    pub context_id: &'a systemprompt_identifiers::ContextId,
    pub task: &'a Task,
    pub agent_message: &'a Message,
    pub response_text: &'a str,
}

pub(super) async fn broadcast_agui_lifecycle(params: BroadcastAguiLifecycleParams<'_>) {
    let webhooks = params.webhooks;
    let task_id = params.task.id.clone();
    let message_id = params.agent_message.message_id.clone();

    let start_event =
        AgUiEventBuilder::run_started(params.context_id.clone(), task_id.clone(), None);
    if let Err(e) = webhooks.broadcast_agui(start_event).await {
        tracing::debug!(error = %e, "Failed to broadcast run_started event");
    }

    let msg_start =
        AgUiEventBuilder::text_message_start(message_id.to_string(), AgUiMessageRole::Assistant);
    if let Err(e) = webhooks.broadcast_agui(msg_start).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_start event");
    }

    let msg_content =
        AgUiEventBuilder::text_message_content(message_id.to_string(), params.response_text);
    if let Err(e) = webhooks.broadcast_agui(msg_content).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_content event");
    }

    let msg_end = AgUiEventBuilder::text_message_end(message_id.to_string());
    if let Err(e) = webhooks.broadcast_agui(msg_end).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_end event");
    }

    let result = serde_json::json!({
        "text": params.response_text,
        "artifacts": params.task.artifacts,
    });
    let finish_event =
        AgUiEventBuilder::run_finished(params.context_id.clone(), task_id, Some(result));
    if let Err(e) = webhooks.broadcast_agui(finish_event).await {
        tracing::debug!(error = %e, "Failed to broadcast run_finished event");
    }
}
