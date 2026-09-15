//! The task-completion stream handler.
//!
//! [`handle_complete`] builds and validates the final [`Task`], persists it
//! with its messages — the guarded `Completed` transition commits in the same
//! transaction as the messages — and then broadcasts the A2A, AG-UI and
//! webhook success events. A failure before the commit leaves the task in its
//! `Working` state and is returned as a [`CompletionFailure`] for the event
//! loop to record and announce.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{RequestContext, TaskMetadata};
use systemprompt_traits::validation::Validate;
use tokio::sync::mpsc::Sender;

use super::success::{BroadcastTaskSuccessParams, broadcast_task_success};
use crate::models::a2a::{
    Artifact, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use crate::services::a2a_server::processing::message::{
    MessageProcessor, PersistCompletedTaskOnProcessorParams,
};
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
    pub processor: &'a Arc<MessageProcessor>,
}

#[derive(Debug)]
pub(in crate::services::a2a_server::streaming) struct CompletionFailure {
    pub code: &'static str,
    pub message: String,
}

pub(in crate::services::a2a_server::streaming) async fn handle_complete(
    params: HandleCompleteParams<'_>,
) -> Result<(), CompletionFailure> {
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
        processor,
    } = params;

    let artifacts_for_task = (!artifacts.is_empty()).then(|| artifacts.clone());
    let task_metadata = validated_metadata(agent_name)?;

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
        return Err(CompletionFailure {
            code: "INTERNAL_ERROR",
            message: "Task status message cannot be None".to_owned(),
        });
    };

    let outcome = processor
        .persist_completed_task(PersistCompletedTaskOnProcessorParams {
            task: &complete_task,
            user_message: original_message,
            agent_message: &agent_message,
            context,
            agent_name,
            artifacts_already_published: true,
        })
        .await
        .map_err(|e| CompletionFailure {
            code: "PERSISTENCE_ERROR",
            message: format!("Failed to complete task and persist messages: {e}"),
        })?;
    outcome.record_undelivered_broadcasts();

    broadcast_task_success(BroadcastTaskSuccessParams {
        tx,
        webhook_context,
        task_id,
        context_id,
        message_id,
        full_text: &full_text,
        artifact_count: artifacts.len(),
        task_with_timing: &outcome.task,
    })
    .await;
    Ok(())
}

fn validated_metadata(agent_name: &str) -> Result<TaskMetadata, CompletionFailure> {
    let task_metadata =
        TaskMetadata::new_validated_agent_message(agent_name.to_owned()).map_err(|e| {
            CompletionFailure {
                code: "METADATA_ERROR",
                message: format!("Internal error: {e}"),
            }
        })?;

    task_metadata.validate().map_err(|e| CompletionFailure {
        code: "VALIDATION_ERROR",
        message: format!("Validation failed: {e}"),
    })?;

    Ok(task_metadata)
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
    let agent_message = Message {
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
    };
    Task {
        id: params.task_id.clone(),
        context_id: params.context_id.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            message: Some(agent_message.clone()),
            timestamp: Some(now),
        },
        history: Some(vec![params.original_message.clone(), agent_message]),
        artifacts: params.artifacts_for_task,
        metadata: Some(params.task_metadata),
        created_at: Some(now),
        last_modified: Some(now),
    }
}
