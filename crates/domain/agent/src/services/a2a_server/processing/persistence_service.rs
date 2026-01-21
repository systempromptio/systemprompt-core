use anyhow::{anyhow, Result};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TaskId, TraceId, UserId};
use systemprompt_models::{RequestContext, TaskMetadata};

use crate::models::{Message, Task, TaskState, TaskStatus};
use crate::repository::task::TaskRepository;
use crate::services::ArtifactPublishingService;

#[derive(Debug)]
pub struct PersistenceService {
    db_pool: DbPool,
}

impl PersistenceService {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn create_task(
        &self,
        task: &Task,
        context: &RequestContext,
        agent_name: &str,
    ) -> Result<()> {
        let task_repo = TaskRepository::new(self.db_pool.clone());

        task_repo
            .create_task(
                task,
                &UserId::new(context.user_id().as_str()),
                &SessionId::new(context.session_id().as_str()),
                &TraceId::new(context.trace_id().as_str()),
                agent_name,
            )
            .await
            .map_err(|e| anyhow!("Failed to persist task at start: {}", e))?;

        tracing::info!(task_id = %task.id, "Task persisted to database");

        Ok(())
    }

    pub async fn update_task_state(
        &self,
        task_id: &TaskId,
        state: TaskState,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let task_repo = TaskRepository::new(self.db_pool.clone());
        task_repo
            .update_task_state(task_id, state, timestamp)
            .await
            .map_err(|e| anyhow!("Failed to update task state: {}", e))
    }

    pub async fn persist_completed_task(
        &self,
        task: &Task,
        user_message: &Message,
        agent_message: &Message,
        context: &RequestContext,
        artifacts_already_published: bool,
    ) -> Result<Task> {
        let task_repo = TaskRepository::new(self.db_pool.clone());

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
                let publishing_service = ArtifactPublishingService::new(self.db_pool.clone());
                for artifact in artifacts {
                    publishing_service
                        .publish_from_a2a(artifact, &task.id, &task.context_id)
                        .await
                        .map_err(|e| {
                            anyhow!("Failed to publish artifact {}: {}", artifact.id, e)
                        })?;
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

    pub fn build_initial_task(
        &self,
        task_id: TaskId,
        context_id: systemprompt_identifiers::ContextId,
        agent_name: &str,
    ) -> Task {
        let metadata = TaskMetadata::new_agent_message(agent_name.to_string());

        Task {
            id: task_id,
            context_id,
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: Some(metadata),
            kind: "task".to_string(),
        }
    }
}
