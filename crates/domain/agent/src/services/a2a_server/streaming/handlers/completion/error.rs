//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder};
use tokio::sync::mpsc::Sender;

use super::send_a2a_status_event;
use crate::models::a2a::{TaskState, TaskStatus};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;

pub(in crate::services::a2a_server::streaming) struct HandleErrorParams<'a> {
    pub tx: &'a Sender<Event>,
    pub webhook_context: &'a WebhookContext,
    pub error: String,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub task_repo: &'a TaskRepository,
}

pub(in crate::services::a2a_server::streaming) async fn handle_error(
    params: HandleErrorParams<'_>,
) {
    let HandleErrorParams {
        tx,
        webhook_context,
        error,
        task_id,
        context_id,
        task_repo,
    } = params;
    tracing::error!(task_id = %task_id, error = %error, "Stream error");

    let failed_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_failed_with_error(task_id, &error, &failed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task to failed state");
    }

    let failed_status = TaskStatus {
        state: TaskState::Failed,
        message: None,
        timestamp: Some(chrono::Utc::now()),
    };
    send_a2a_status_event(tx, task_id, context_id, failed_status, true);

    let a2a_event = A2AEventBuilder::task_status_update(
        task_id.clone(),
        context_id.clone(),
        TaskState::Failed,
        Some(error.clone()),
    );
    if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
        tracing::error!(error = %e, "Failed to broadcast A2A task_status_update");
    }

    let error_event = AgUiEventBuilder::run_error(error, Some("STREAM_ERROR".to_owned()));
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}
