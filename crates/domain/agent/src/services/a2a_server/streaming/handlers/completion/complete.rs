//! The task-completion stream handler.
//!
//! [`handle_complete`] marks the task completed, builds and validates the final
//! [`Task`], persists it with its messages, and broadcasts the A2A, AG-UI, and
//! webhook success events; failures along the way are recorded and reported as
//! AG-UI `RUN_ERROR` events.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{AgUiEventBuilder, RequestContext, TaskMetadata};
use systemprompt_traits::validation::Validate;
use tokio::sync::mpsc::Sender;

use super::success::{BroadcastTaskSuccessParams, broadcast_task_success};
use crate::models::a2a::{
    Artifact, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::processing::message::{
    MessageProcessor, PersistCompletedTaskOnProcessorParams,
};
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;
use crate::services::shared::AgentServiceError;

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
    mark_task_completed(task_repo, task_id).await;

    let artifacts_for_task = (!artifacts.is_empty()).then(|| artifacts.clone());

    let Some(task_metadata) = resolve_validated_metadata(agent_name, webhook_context).await else {
        return;
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

    let Some(agent_message) = complete_task.status.message.clone() else {
        tracing::error!("Task status message is None");
        report_run_error(
            webhook_context,
            "Task status message cannot be None".to_owned(),
            "INTERNAL_ERROR",
        )
        .await;
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
            handle_persistence_failure(task_repo, task_id, webhook_context, &e).await;
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

async fn mark_task_completed(task_repo: &TaskRepository, task_id: &TaskId) {
    let completed_timestamp = chrono::Utc::now();
    if let Err(e) = task_repo
        .update_task_state(task_id, TaskState::Completed, &completed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task state");
    }
}

async fn resolve_validated_metadata(
    agent_name: &str,
    webhook_context: &WebhookContext,
) -> Option<TaskMetadata> {
    let task_metadata = match TaskMetadata::new_validated_agent_message(agent_name.to_owned()) {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create TaskMetadata");
            report_run_error(
                webhook_context,
                format!("Internal error: {e}"),
                "METADATA_ERROR",
            )
            .await;
            return None;
        },
    };

    if let Err(e) = task_metadata.validate() {
        tracing::error!(error = %e, "Task metadata validation failed");
        report_run_error(
            webhook_context,
            format!("Validation failed: {e}"),
            "VALIDATION_ERROR",
        )
        .await;
        return None;
    }

    Some(task_metadata)
}

async fn report_run_error(webhook_context: &WebhookContext, message: String, code: &str) {
    let error_event = AgUiEventBuilder::run_error(message, Some(code.to_owned()));
    if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
    }
}

async fn handle_persistence_failure(
    task_repo: &TaskRepository,
    task_id: &TaskId,
    webhook_context: &WebhookContext,
    e: &AgentServiceError,
) {
    let error_msg = format!("Failed to complete task and persist messages: {}", e);
    tracing::error!(task_id = %task_id, error = %e, "Failed to complete task and persist messages");

    let failed_timestamp = chrono::Utc::now();
    if let Err(update_err) = task_repo
        .update_task_failed_with_error(task_id, &error_msg, &failed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %update_err, "Failed to update task to failed state");
    }

    report_run_error(
        webhook_context,
        format!("Failed to persist task: {e}"),
        "PERSISTENCE_ERROR",
    )
    .await;
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
