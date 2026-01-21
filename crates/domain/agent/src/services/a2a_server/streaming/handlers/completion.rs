use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder, RequestContext, TaskMetadata};
use systemprompt_traits::validation::Validate;
use tokio::sync::mpsc::UnboundedSender;

use crate::models::a2a::protocol::TaskStatusUpdateEvent;
use crate::models::a2a::{Artifact, Message, Part, Task, TaskState, TaskStatus, TextPart};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::processing::message::MessageProcessor;
use crate::services::a2a_server::streaming::broadcast_task_completed;
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;

pub struct HandleCompleteParams<'a> {
    pub tx: &'a UnboundedSender<Event>,
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

fn send_a2a_status_event(
    tx: &UnboundedSender<Event>,
    task_id: &TaskId,
    context_id: &ContextId,
    status: TaskStatus,
    is_final: bool,
) {
    let event = TaskStatusUpdateEvent::new(task_id.as_str(), context_id.as_str(), status, is_final);
    let jsonrpc = event.to_jsonrpc_response();
    if tx.send(Event::default().data(jsonrpc.to_string())).is_err() {
        tracing::trace!("Failed to send status event, channel closed");
    }
}

pub async fn handle_complete(params: HandleCompleteParams<'_>) {
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
        .update_task_state(&task_id, TaskState::Completed, &completed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to update task state");
    }

    let artifacts_for_task = if artifacts.is_empty() {
        None
    } else {
        Some(artifacts.clone())
    };

    let task_metadata = match TaskMetadata::new_validated_agent_message(agent_name.to_string()) {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create TaskMetadata");
            let error_event = AgUiEventBuilder::run_error(
                format!("Internal error: {e}"),
                Some("METADATA_ERROR".to_string()),
            );
            if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
                tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
            }
            return;
        },
    };

    let complete_task = Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        kind: "task".to_string(),
        status: TaskStatus {
            state: TaskState::Completed,
            message: Some(Message {
                role: "agent".to_string(),
                parts: vec![Part::Text(TextPart {
                    text: full_text.clone(),
                })],
                id: message_id.to_string().into(),
                task_id: Some(task_id.clone()),
                context_id: context_id.clone(),
                kind: "message".to_string(),
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            }),
            timestamp: Some(chrono::Utc::now()),
        },
        history: Some(vec![
            original_message.clone(),
            Message {
                role: "agent".to_string(),
                parts: vec![Part::Text(TextPart {
                    text: full_text.clone(),
                })],
                id: MessageId::generate(),
                task_id: Some(task_id.clone()),
                context_id: context_id.clone(),
                kind: "message".to_string(),
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            },
        ]),
        artifacts: artifacts_for_task,
        metadata: Some(task_metadata),
    };

    if let Some(ref metadata) = complete_task.metadata {
        if let Err(e) = metadata.validate() {
            tracing::error!(error = %e, "Task metadata validation failed");
            let error_event = AgUiEventBuilder::run_error(
                format!("Validation failed: {e}"),
                Some("VALIDATION_ERROR".to_string()),
            );
            if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
                tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
            }
            return;
        }
    }

    let agent_message = match complete_task.status.message.clone() {
        Some(msg) => msg,
        None => {
            tracing::error!("Task status message is None");
            let error_event = AgUiEventBuilder::run_error(
                "Task status message cannot be None".to_string(),
                Some("INTERNAL_ERROR".to_string()),
            );
            if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
                tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
            }
            return;
        },
    };

    match processor
        .persist_completed_task(
            &complete_task,
            original_message,
            &agent_message,
            context,
            agent_name,
            true,
        )
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
                Some("PERSISTENCE_ERROR".to_string()),
            );
            if let Err(broadcast_err) = webhook_context.broadcast_agui(error_event).await {
                tracing::error!(error = %broadcast_err, "Failed to broadcast RUN_ERROR");
            }
        },
        Ok(task_with_timing) => {
            let completed_status = TaskStatus {
                state: TaskState::Completed,
                message: Some(Message {
                    role: "agent".to_string(),
                    parts: vec![Part::Text(TextPart {
                        text: full_text.clone(),
                    })],
                    id: message_id.to_string().into(),
                    task_id: Some(task_id.clone()),
                    context_id: context_id.clone(),
                    kind: "message".to_string(),
                    metadata: None,
                    extensions: None,
                    reference_task_ids: None,
                }),
                timestamp: Some(chrono::Utc::now()),
            };
            send_a2a_status_event(tx, task_id, context_id, completed_status, true);

            let a2a_event = A2AEventBuilder::task_status_update(
                task_id.clone(),
                context_id.clone(),
                TaskState::Completed,
                Some(full_text.clone()),
            );
            if let Err(e) = webhook_context.broadcast_a2a(a2a_event).await {
                tracing::error!(error = %e, "Failed to broadcast A2A task_status_update");
            }

            let agui_result = serde_json::json!({
                "text": full_text,
                "artifactCount": artifacts.len(),
                "taskId": task_id.as_str(),
                "contextId": context_id.as_str()
            });
            let event = AgUiEventBuilder::run_finished(
                context_id.clone(),
                task_id.clone(),
                Some(agui_result),
            );
            if let Err(e) = webhook_context.broadcast_agui(event).await {
                tracing::error!(error = %e, "Failed to broadcast RUN_FINISHED");
            }

            broadcast_task_completed(&task_with_timing, context.user_id().as_str(), auth_token)
                .await;
        },
    }
}

pub async fn handle_error(
    tx: &UnboundedSender<Event>,
    webhook_context: &WebhookContext,
    error: String,
    task_id: &TaskId,
    context_id: &ContextId,
    task_repo: &TaskRepository,
) {
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

    let error_event = AgUiEventBuilder::run_error(error, Some("STREAM_ERROR".to_string()));
    if let Err(e) = webhook_context.broadcast_agui(error_event).await {
        tracing::error!(error = %e, "Failed to broadcast RUN_ERROR");
    }
}
