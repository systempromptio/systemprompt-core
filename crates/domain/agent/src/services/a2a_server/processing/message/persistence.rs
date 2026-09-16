//! Message persistence during A2A processing.
//!
//! Persisting the task and its messages is the operation; the webhook
//! broadcasts that follow are a side channel whose failures are reported in
//! the [`PersistOutcome`] rather than turned into a persistence error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::ArtifactId;
use systemprompt_models::RequestContext;

use crate::models::a2a::{Message, Task};
use crate::repository::A2ARepositories;
use crate::repository::task::UpdateTaskAndSaveMessagesParams;
use crate::services::ArtifactPublishingService;
use crate::services::a2a_server::streaming::broadcast::{
    broadcast_artifact_created, broadcast_task_completed,
};
use crate::services::a2a_server::streaming::webhook_client::{
    DynWebhookBroadcaster, WebhookContext, WebhookError,
};

/// The persisted task plus the artifact broadcasts that could not be
/// delivered. The task is committed whether or not the list is empty; the
/// consumer records each undelivered broadcast, never this module.
#[derive(Debug)]
pub struct PersistOutcome {
    pub task: Task,
    pub undelivered_broadcasts: Vec<(ArtifactId, WebhookError)>,
}

impl PersistOutcome {
    pub fn record_undelivered_broadcasts(&self) {
        for (artifact_id, error) in &self.undelivered_broadcasts {
            tracing::warn!(
                artifact_id = %artifact_id,
                task_id = %self.task.id,
                error = %error,
                "artifact persisted but its broadcast was not delivered"
            );
        }
    }
}

pub struct PersistCompletedTaskParams<'a> {
    pub task: &'a Task,
    pub user_message: &'a Message,
    pub agent_message: &'a Message,
    pub context: &'a RequestContext,
    pub repositories: &'a A2ARepositories,
    pub publishing: &'a ArtifactPublishingService,
    pub webhooks: DynWebhookBroadcaster,
    pub artifacts_already_published: bool,
}

impl std::fmt::Debug for PersistCompletedTaskParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistCompletedTaskParams")
            .field("task", &self.task.id)
            .field(
                "artifacts_already_published",
                &self.artifacts_already_published,
            )
            .finish_non_exhaustive()
    }
}

pub async fn persist_completed_task(
    params: PersistCompletedTaskParams<'_>,
) -> Result<PersistOutcome> {
    let PersistCompletedTaskParams {
        task,
        user_message,
        agent_message,
        context,
        repositories,
        publishing,
        webhooks,
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
            AgentServiceError::Internal(format!("Failed to update task and save messages: {e}"))
        })?;

    let mut undelivered_broadcasts = Vec::new();
    if !artifacts_already_published && let Some(artifacts) = &task.artifacts {
        let context_id = &task.context_id;
        let webhooks = WebhookContext::for_request(webhooks, context);
        for artifact in artifacts {
            publishing
                .publish_from_a2a(artifact, &task.id, context_id, context.user_id())
                .await
                .map_err(|e| {
                    AgentServiceError::Internal(format!(
                        "Failed to publish artifact {}: {e}",
                        artifact.id
                    ))
                })?;

            if let Err(e) =
                broadcast_artifact_created(&webhooks, artifact, &task.id, context_id).await
            {
                undelivered_broadcasts.push((artifact.id.clone(), e));
            }
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

    Ok(PersistOutcome {
        task: updated_task,
        undelivered_broadcasts,
    })
}

pub async fn broadcast_completion(
    webhooks: DynWebhookBroadcaster,
    task: &Task,
    context: &RequestContext,
) {
    let webhooks = WebhookContext::for_request(webhooks, context);
    broadcast_task_completed(&webhooks, task).await;
}
