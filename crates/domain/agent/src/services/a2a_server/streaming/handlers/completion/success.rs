//! Success fan-out for a completed task: the final A2A status frame, the A2A
//! and AG-UI webhook events, and the task-completed broadcast.

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder, RequestContext};
use tokio::sync::mpsc::Sender;

use super::send_a2a_status_event;
use crate::models::a2a::{Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart};
use crate::services::a2a_server::streaming::broadcast_task_completed;
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;

pub(super) struct BroadcastTaskSuccessParams<'a> {
    pub tx: &'a Sender<Event>,
    pub webhook_context: &'a WebhookContext,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub message_id: &'a str,
    pub full_text: &'a str,
    pub artifact_count: usize,
    pub task_with_timing: &'a Task,
    pub context: &'a RequestContext,
    pub auth_token: &'a str,
}

pub(super) async fn broadcast_task_success(params: BroadcastTaskSuccessParams<'_>) {
    let completed_status = TaskStatus {
        state: TaskState::Completed,
        message: Some(Message {
            role: MessageRole::Agent,
            parts: vec![Part::Text(TextPart {
                text: params.full_text.to_owned(),
            })],
            message_id: MessageId::new(params.message_id.to_owned()),
            task_id: Some(params.task_id.clone()),
            context_id: params.context_id.clone(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        }),
        timestamp: Some(chrono::Utc::now()),
    };
    send_a2a_status_event(
        params.tx,
        params.task_id,
        params.context_id,
        completed_status,
        true,
    );

    let a2a_event = A2AEventBuilder::task_status_update(
        params.task_id.clone(),
        params.context_id.clone(),
        TaskState::Completed,
        Some(params.full_text.to_owned()),
    );
    if let Err(e) = params.webhook_context.broadcast_a2a(a2a_event).await {
        tracing::error!(error = %e, "Failed to broadcast A2A task_status_update");
    }

    let agui_result = serde_json::json!({
        "text": params.full_text,
        "artifactCount": params.artifact_count,
        "taskId": params.task_id.as_str(),
        "contextId": params.context_id.as_str()
    });
    let event = AgUiEventBuilder::run_finished(
        params.context_id.clone(),
        params.task_id.clone(),
        Some(agui_result),
    );
    if let Err(e) = params.webhook_context.broadcast_agui(event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_FINISHED");
    }

    broadcast_task_completed(
        params.task_with_timing,
        params.context.user_id(),
        params.auth_token,
    )
    .await;
}
