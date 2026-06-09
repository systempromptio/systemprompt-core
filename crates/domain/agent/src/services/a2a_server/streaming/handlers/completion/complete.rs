//! The task-completion stream handler.
//!
//! [`handle_complete`] marks the task completed, builds and validates the final
//! [`Task`], persists it with its messages, and broadcasts the A2A, AG-UI, and
//! webhook success events; failures along the way are recorded and reported as
//! AG-UI `RUN_ERROR` events.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder, RequestContext, TaskMetadata};
use systemprompt_traits::validation::Validate;
use tokio::sync::mpsc::Sender;

use super::send_a2a_status_event;
use crate::models::a2a::{
    Artifact, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::processing::message::{
    MessageProcessor, PersistCompletedTaskOnProcessorParams,
};
use crate::services::a2a_server::streaming::broadcast_task_completed;
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;

pub(in crate::services::a2a_server::streaming) struct HandleCompleteParams<'a> {
    pub tx: &'a Sender<Event>,
    pub webhook_context: &'a WebhookContext,
    pub full_text: String,
    pub artifacts: Vec<Artifact>,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub id: &'a str,
    pub original_message: &'a Message,
    pub agent_name: &'a str,
    pub context: &'a RequestContext,
    pub auth_token: &'a str,
    pub task_repo: &'a TaskRepository,
    pub processor: &'a Arc<MessageProcessor>,
}

pub(in crate::services::a2a_server::streaming) async fn handle_complete(
    params: HandleCompleteParams<'_>,
) {
    let HandleCompleteParams {
        tx,
        webhook_context,
        full_text,
        artifacts,
        task_id,
        context_id,
        id: message_id,
        original_message,
        agent_name,
        context,
        auth_token,
        task_repo,
        processor,
    } = params;
    let completed_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_state(task_id, TaskState::Completed, &completed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task state");
    }

    let artifacts_for_task = if artifacts.is_empty() {
        None
    } else {
        Some(artifacts.clone())
    };

    let task_metadata = match TaskMetadata::new_validated_agent_message(agent_name.to_owned()) {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create TaskMetadata");
            let error_event = AgUiEventBuilder::run_error(
                format!("Internal error: {e}"),
                Some("METADATA_ERROR".to_owned()),
            );
            if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
                tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
            }
            return;
        },
    };

    let complete_task = build_complete_task(BuildCompleteTaskParams {
        task_id,
        context_id,
        message_id,
        full_text: &full_text,
        original_message,
        artifacts_for_task,
        task_metadata,
    });

    if let Some(ref metadata) = complete_task.metadata
        && let Err(e) = metadata.validate()
    {
        tracing::error!(error = %e, "Task metadata validation failed");
        let error_event = AgUiEventBuilder::run_error(
            format!("Validation failed: {e}"),
            Some("VALIDATION_ERROR".to_owned()),
        );
        if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
            tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
        }
        return;
    }

    let Some(agent_message) = complete_task.status.message.clone() else {
        tracing::error!("Task status message is None");
        let error_event = AgUiEventBuilder::run_error(
            "Task status message cannot be None".to_owned(),
            Some("INTERNAL_ERROR".to_owned()),
        );
        if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
            tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
        }
        return;
    };

    match processor
        .persist_completed_task(PersistCompletedTaskOnProcessorParams {
            task: &complete_task,
            user_message: original_message,
            agent_message: &agent_message,
            context,
            agent_name,
            artifacts_already_published: true,
        })
        .await
    {
        Err(e) => {
            let error_msg = format!("Failed to complete task and persist messages: {}", e);
            tracing::error!(task_id = %task_id, error = %e, "Failed to complete task and persist messages");

            let failed_timestamp = chrono::Utc::now();
            if let Err(update_err) = task_repo
                .update_task_failed_with_error(task_id, &error_msg, &failed_timestamp)
                .await
            {
                tracing::error!(task_id = %task_id, error = %update_err, "Failed to update task to failed state");
            }

            let error_event = AgUiEventBuilder::run_error(
                format!("Failed to persist task: {e}"),
                Some("PERSISTENCE_ERROR".to_owned()),
            );
            if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
                tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
            }
        },
        Ok(task_with_timing) => {
            broadcast_task_success(BroadcastTaskSuccessParams {
                tx,
                webhook_context,
                task_id,
                context_id,
                message_id,
                full_text: &full_text,
                artifact_count: artifacts.len(),
                task_with_timing: &task_with_timing,
                context,
                auth_token,
            })
            .await;
        },
    }
}

struct BuildCompleteTaskParams<'a> {
    task_id: &'a TaskId,
    context_id: &'a ContextId,
    message_id: &'a str,
    full_text: &'a str,
    original_message: &'a Message,
    artifacts_for_task: Option<Vec<Artifact>>,
    task_metadata: TaskMetadata,
}

fn build_complete_task(params: BuildCompleteTaskParams<'_>) -> Task {
    let now = chrono::Utc::now();
    Task {
        id: params.task_id.clone(),
        context_id: params.context_id.clone(),
        status: TaskStatus {
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
            timestamp: Some(now),
        },
        history: Some(vec![
            params.original_message.clone(),
            Message {
                role: MessageRole::Agent,
                parts: vec![Part::Text(TextPart {
                    text: params.full_text.to_owned(),
                })],
                message_id: MessageId::generate(),
                task_id: Some(params.task_id.clone()),
                context_id: params.context_id.clone(),
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            },
        ]),
        artifacts: params.artifacts_for_task,
        metadata: Some(params.task_metadata),
        created_at: Some(now),
        last_modified: Some(now),
    }
}

struct BroadcastTaskSuccessParams<'a> {
    tx: &'a Sender<Event>,
    webhook_context: &'a WebhookContext,
    task_id: &'a TaskId,
    context_id: &'a ContextId,
    message_id: &'a str,
    full_text: &'a str,
    artifact_count: usize,
    task_with_timing: &'a Task,
    context: &'a RequestContext,
    auth_token: &'a str,
}

async fn broadcast_task_success(params: BroadcastTaskSuccessParams<'_>) {
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
