use anyhow::{anyhow, Result};
use systemprompt_database::DbPool;
use systemprompt_models::RequestContext;

use crate::models::a2a::{Message, Task};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::streaming::{
    broadcast_artifact_created, broadcast_task_completed,
};
use crate::services::ArtifactPublishingService;

pub async fn persist_completed_task(
    task: &Task,
    user_message: &Message,
    agent_message: &Message,
    context: &RequestContext,
    task_repo: &TaskRepository,
    db_pool: &DbPool,
    artifacts_already_published: bool,
) -> Result<Task> {
    let updated_task = task_repo
        .update_task_and_save_messages(
            task,
            user_message,
            agent_message,
            Some(context.user_id()),
            context.session_id(),
            context.trace_id(),
        )
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
