//! Message-handler helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::{AgUiEventBuilder, AgUiMessageRole, RequestContext};

use crate::models::a2a::{Artifact, Message, Task};
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::a2a_server::streaming::webhook_client::broadcast_agui_event;
use crate::services::shared::{AgentServiceError, Result};

pub(super) async fn collect_stream_response(
    mut chunk_rx: tokio::sync::mpsc::Receiver<StreamEvent>,
    context: &RequestContext,
) -> Result<(String, Vec<Artifact>)> {
    let mut response_text = String::new();
    let mut tool_artifacts = Vec::new();

    while let Some(event) = chunk_rx.recv().await {
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
                if let Err(e) = broadcast_agui_event(
                    context.user_id(),
                    error_event,
                    context.auth_token().as_str(),
                )
                .await
                {
                    tracing::debug!(error = %e, "Failed to broadcast error event");
                }
                return Err(AgentServiceError::Internal(error.clone()));
            },
            _ => {},
        }
    }

    Ok((response_text, tool_artifacts))
}

pub(super) struct BroadcastAguiLifecycleParams<'a> {
    pub context: &'a RequestContext,
    pub context_id: &'a systemprompt_identifiers::ContextId,
    pub task: &'a Task,
    pub agent_message: &'a Message,
    pub response_text: &'a str,
}

pub(super) async fn broadcast_agui_lifecycle(params: BroadcastAguiLifecycleParams<'_>) {
    let user_id = params.context.user_id();
    let auth_token = params.context.auth_token().as_str();
    let task_id = params.task.id.clone();
    let message_id = params.agent_message.message_id.clone();

    let start_event =
        AgUiEventBuilder::run_started(params.context_id.clone(), task_id.clone(), None);
    if let Err(e) = broadcast_agui_event(user_id, start_event, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast run_started event");
    }

    let msg_start =
        AgUiEventBuilder::text_message_start(message_id.to_string(), AgUiMessageRole::Assistant);
    if let Err(e) = broadcast_agui_event(user_id, msg_start, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_start event");
    }

    let msg_content =
        AgUiEventBuilder::text_message_content(message_id.to_string(), params.response_text);
    if let Err(e) = broadcast_agui_event(user_id, msg_content, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_content event");
    }

    let msg_end = AgUiEventBuilder::text_message_end(message_id.to_string());
    if let Err(e) = broadcast_agui_event(user_id, msg_end, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_end event");
    }

    let result = serde_json::json!({
        "text": params.response_text,
        "artifacts": params.task.artifacts,
    });
    let finish_event =
        AgUiEventBuilder::run_finished(params.context_id.clone(), task_id, Some(result));
    if let Err(e) = broadcast_agui_event(user_id, finish_event, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast run_finished event");
    }
}
