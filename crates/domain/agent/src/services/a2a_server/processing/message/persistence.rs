use anyhow::{Result, anyhow};
use systemprompt_database::DbPool;
use systemprompt_models::RequestContext;

use crate::models::a2a::{Message, Task};
use crate::repository::task::{TaskRepository, UpdateTaskAndSaveMessagesParams};
use crate::services::ArtifactPublishingService;
use crate::services::a2a_server::streaming::{
    broadcast_artifact_created, broadcast_task_completed,
};

pub struct PersistCompletedTaskParams<'a> {
    pub task: &'a Task,
    pub user_message: &'a Message,
    pub agent_message: &'a Message,
    pub context: &'a RequestContext,
    pub task_repo: &'a TaskRepository,
    pub db_pool: &'a DbPool,
    pub artifacts_already_published: bool,
}

pub async fn persist_completed_task(params: PersistCompletedTaskParams<'_>) -> Result<Task> {
    let PersistCompletedTaskParams {
        task,
        user_message,
        agent_message,
        context,
        task_repo,
        db_pool,
        artifacts_already_published,
    } = params;
    let updated_task = task_repo
        .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
            task,
            user_message,
            agent_message,
            user_id: Some(context.user_id()),
            session_id: context.session_id(),
            trace_id: context.trace_id(),
        })
        .await
        .map_err(|e| anyhow!("Failed to update task and save messages: {}", e))?;

    if !artifacts_already_published {
        if let Some(ref artifacts) = task.artifacts {
            let publishing_service = ArtifactPublishingService::new(db_pool)?;
            for artifact in artifacts {
                publishing_service
                    .publish_from_a2a(artifact, &task.id, &task.context_id)
                    .await
                    .map_err(|e| anyhow!("Failed to publish artifact {}: {}", artifact.id, e))?;

                broadcast_artifact_created(
                    artifact,
                    &task.id,
                    &task.context_id,
                    context.user_id().as_str(),
                    context.auth_token().as_str(),
                )
                .await
                .map_err(|e| anyhow!("Failed to broadcast artifact {}: {}", artifact.id, e))?;
            }

            tracing::info!(
                task_id = %task.id,
                artifact_count = artifacts.len(),
                "Published artifacts for task"
            );
        }
    }

    tracing::info!(
        task_id = %task.id,
        context_id = %task.context_id,
        user_id = %context.user_id(),
        "Persisted task"
    );

    Ok(updated_task)
}

pub async fn broadcast_completion(task: &Task, context: &RequestContext) {
    broadcast_task_completed(
        task,
        context.user_id().as_str(),
        context.auth_token().as_str(),
    )
    .await;
}
