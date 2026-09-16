//! Task and artifact lifecycle broadcasts to the internal webhook.
//!
//! The task-event broadcasts are best-effort side channels: a delivery
//! failure is logged and never fails the task. [`broadcast_artifact_created`]
//! returns the failure so the caller can record it as a typed partial outcome.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::TaskMetadata;

use super::webhook_client::{LifecycleEvent, WebhookContext, WebhookError};
use crate::models::a2a::{Artifact, Message, Task, TaskState, TaskStatus};

#[derive(Debug)]
pub struct BroadcastTaskCreatedParams<'a> {
    pub webhooks: &'a WebhookContext,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub user_message: &'a Message,
    pub agent_name: &'a str,
}

pub async fn broadcast_task_created(params: BroadcastTaskCreatedParams<'_>) {
    let BroadcastTaskCreatedParams {
        webhooks,
        task_id,
        context_id,
        user_message,
        agent_name,
    } = params;
    let event_task = build_event_task(task_id, context_id, user_message, agent_name);

    let event = match LifecycleEvent::task_created(&event_task, webhooks.user_id()) {
        Ok(event) => event,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to serialize task for broadcast");
            return;
        },
    };

    match webhooks.broadcast_lifecycle(event).await {
        Ok(()) => tracing::info!(task_id = %task_id, "Broadcast task_created via webhook"),
        Err(e) => tracing::warn!(task_id = %task_id, error = %e, "Webhook broadcast failed"),
    }
}

pub async fn broadcast_task_completed(webhooks: &WebhookContext, task: &Task) {
    let event = match LifecycleEvent::task_completed(task, webhooks.user_id()) {
        Ok(event) => event,
        Err(e) => {
            tracing::warn!(task_id = %task.id, error = %e, "Failed to serialize task for broadcast");
            return;
        },
    };

    match webhooks.broadcast_lifecycle(event).await {
        Ok(()) => tracing::info!(task_id = %task.id, "Broadcast task_completed"),
        Err(e) => tracing::warn!(task_id = %task.id, error = %e, "Webhook broadcast failed"),
    }
}

fn build_event_task(
    task_id: &TaskId,
    context_id: &ContextId,
    user_message: &Message,
    agent_name: &str,
) -> Task {
    Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: Some(vec![user_message.clone()]),
        artifacts: None,
        metadata: Some(TaskMetadata::new_agent_message(agent_name.to_owned())),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}

pub async fn broadcast_artifact_created(
    webhooks: &WebhookContext,
    artifact: &Artifact,
    task_id: &TaskId,
    context_id: &ContextId,
) -> Result<(), WebhookError> {
    let event = LifecycleEvent::artifact_created(&artifact.id, context_id, webhooks.user_id());
    webhooks.broadcast_lifecycle(event).await?;
    tracing::info!(
        artifact_id = %artifact.id,
        task_id = %task_id,
        "Broadcast artifact_created via webhook"
    );
    Ok(())
}
