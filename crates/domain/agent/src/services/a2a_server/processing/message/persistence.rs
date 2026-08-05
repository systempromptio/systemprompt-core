//! Message persistence during A2A processing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use systemprompt_models::RequestContext;

use crate::models::a2a::{Message, Task};
use crate::repository::A2ARepositories;
use crate::repository::task::UpdateTaskAndSaveMessagesParams;
use crate::services::ArtifactPublishingService;
use crate::services::a2a_server::streaming::{
    broadcast_artifact_created, broadcast_task_completed,
};

pub(super) struct PersistCompletedTaskParams<'a> {
    pub task: &'a Task,
    pub user_message: &'a Message,
    pub agent_message: &'a Message,
    pub context: &'a RequestContext,
    pub repositories: &'a A2ARepositories,
    pub artifacts_already_published: bool,
}

pub(super) async fn persist_completed_task(params: PersistCompletedTaskParams<'_>) -> Result<Task> {
    let PersistCompletedTaskParams {
        task,
        user_message,
        agent_message,
        context,
        repositories,
        artifacts_already_published,
    } = params;
    let updated_task = repositories
        .tasks
        .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
            task,
            user_message,
            agent_message,
            user_id: Some(context.user_id()),
            session_id: context.session_id(),
            trace_id: context.trace_id(),
        })
        .await
        .map_err(|e| {
            AgentServiceError::Internal(format!("Failed to update task and save messages: {}", e))
        })?;

    if !artifacts_already_published && let Some(artifacts) = &task.artifacts {
        let context_id = &task.context_id;
        let publishing_service = ArtifactPublishingService::new(
            repositories.artifacts.clone(),
            repositories.execution_steps.clone(),
            repositories.tasks.clone(),
        )?;
        for artifact in artifacts {
            publishing_service
                .publish_from_a2a(artifact, &task.id, context_id)
                .await
                .map_err(|e| {
                    AgentServiceError::Internal(format!(
                        "Failed to publish artifact {}: {}",
                        artifact.id, e
                    ))
                })?;

            broadcast_artifact_created(
                artifact,
                &task.id,
                context_id,
                context.user_id(),
                context.auth_token().as_str(),
            )
            .await
            .map_err(|e| {
                AgentServiceError::Internal(format!(
                    "Failed to broadcast artifact {}: {}",
                    artifact.id, e
                ))
            })?;
        }

        tracing::info!(
            task_id = %task.id,
            artifact_count = artifacts.len(),
            "Published artifacts for task"
        );
    }

    tracing::info!(
        task_id = %task.id,
        context_id = ?task.context_id,
        user_id = %context.user_id(),
        "Persisted task"
    );

    Ok(updated_task)
}

pub(super) async fn broadcast_completion(task: &Task, context: &RequestContext) {
    broadcast_task_completed(task, context.user_id(), context.auth_token().as_str()).await;
}
