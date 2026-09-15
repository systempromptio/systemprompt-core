//! Stream failure and cancellation handling: the task-state write and the
//! terminal announcements are separate steps so a failure discovered after
//! the state was already written (persistence) is announced exactly once.
//!
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

pub(in crate::services::a2a_server::streaming) async fn record_failure(
    task_repo: &TaskRepository,
    task_id: &TaskId,
    error: &str,
) {
    tracing::error!(task_id = %task_id, error = %error, "Stream error");

    let failed_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_failed_with_error(task_id, error, &failed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task to failed state");
    }
}

pub(in crate::services::a2a_server::streaming) struct AnnounceFailureParams<'a> {
    pub tx: &'a Sender<Event>,
    pub webhook_context: &'a WebhookContext,
    pub error: String,
    pub code: &'a str,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
}

pub(in crate::services::a2a_server::streaming) async fn announce_failure(
    params: AnnounceFailureParams<'_>,
) {
    let AnnounceFailureParams {
        tx,
        webhook_context,
        error,
        code,
        task_id,
        context_id,
    } = params;

    let failed_status = TaskStatus {
        state: TaskState::Failed,
        message: None,
        timestamp: Some(chrono::Utc::now()),
    };
    send_a2a_status_event(tx, task_id, context_id, failed_status, true).await;

    let a2a_event = A2AEventBuilder::task_status_update(
        task_id.clone(),
        context_id.clone(),
        TaskState::Failed,
        Some(error.clone()),
    );
    if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
        tracing::error!(error = %e, "Failed to broadcast A2A task_status_update");
    }

    let error_event = AgUiEventBuilder::run_error(error, Some(code.to_owned()));
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}

pub(in crate::services::a2a_server::streaming) async fn announce_cancelled(
    tx: &Sender<Event>,
    webhook_context: &WebhookContext,
    task_id: &TaskId,
    context_id: &ContextId,
) {
    let canceled_status = TaskStatus {
        state: TaskState::Canceled,
        message: None,
        timestamp: Some(chrono::Utc::now()),
    };
    send_a2a_status_event(tx, task_id, context_id, canceled_status, true).await;

    let a2a_event = A2AEventBuilder::task_status_update(
        task_id.clone(),
        context_id.clone(),
        TaskState::Canceled,
        None,
    );
    if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
        tracing::error!(error = %e, "Failed to broadcast A2A task_status_update");
    }

    let error_event = AgUiEventBuilder::run_error(
        "Task was cancelled".to_owned(),
        Some("TASK_CANCELLED".to_owned()),
    );
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}
