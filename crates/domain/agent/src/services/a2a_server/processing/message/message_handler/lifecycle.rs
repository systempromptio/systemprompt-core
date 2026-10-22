//! Task persistence around a non-streaming message: the submitted task is
//! stored and announced before the pipeline runs, and the completed task is
//! persisted and broadcast after it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AgentName;
use systemprompt_models::RequestContext;

use super::stream_collection::{BroadcastAguiLifecycleParams, broadcast_agui_lifecycle};
use crate::models::a2a::{Message, Task, TaskState};
use crate::services::a2a_server::processing::message::MessageHandler;
use crate::services::a2a_server::processing::message::persistence::{
    PersistOutcome, broadcast_completion, persist_completed_task,
};
use crate::services::a2a_server::streaming::broadcast::{
    BroadcastTaskCreatedParams, broadcast_task_created,
};
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;
use crate::services::shared::{AgentServiceError, Result};

pub(super) struct FinalizeParams<'a> {
    pub task: &'a Task,
    pub message: &'a Message,
    pub agent_message: &'a Message,
    pub response_text: &'a str,
    pub context: &'a RequestContext,
    pub webhooks: &'a WebhookContext,
}

pub(super) struct PersistAndAnnounceParams<'a> {
    pub task: &'a Task,
    pub message: &'a Message,
    pub agent_name: &'a AgentName,
    pub context: &'a RequestContext,
    pub webhooks: &'a WebhookContext,
}

impl MessageHandler {
    pub(super) async fn finalize(&self, params: FinalizeParams<'_>) -> Result<()> {
        let FinalizeParams {
            task,
            message,
            agent_message,
            response_text,
            context,
            webhooks,
        } = params;
        if context.user_type() == systemprompt_models::auth::UserType::Anon {
            tracing::warn!(
                context_id = %task.context_id,
                session_id = %context.session_id(),
                "Saving messages for anonymous user"
            );
        }

        self.persist_or_mark_failed(task, message, agent_message, context)
            .await?
            .record_undelivered_broadcasts();

        broadcast_completion(self.webhooks(), task, context).await;

        broadcast_agui_lifecycle(BroadcastAguiLifecycleParams {
            webhooks,
            context_id: &task.context_id,
            task,
            agent_message,
            response_text,
        })
        .await;
        Ok(())
    }

    pub(super) async fn persist_and_announce(
        &self,
        params: PersistAndAnnounceParams<'_>,
    ) -> Result<()> {
        let PersistAndAnnounceParams {
            task,
            message,
            agent_name,
            context,
            webhooks,
        } = params;
        if let Err(e) = self
            .repositories
            .tasks
            .create_task(crate::repository::task::RepoCreateTaskParams {
                task,
                user_id: context.user_id(),
                session_id: context.session_id(),
                trace_id: context.trace_id(),
                agent_name,
            })
            .await
        {
            return Err(AgentServiceError::operation(
                "Failed to persist task at start",
                e,
            ));
        }

        tracing::info!(task_id = %task.id, "Task persisted to database");

        broadcast_task_created(BroadcastTaskCreatedParams {
            webhooks,
            task_id: &task.id,
            context_id: &task.context_id,
            user_message: message,
            agent_name,
        })
        .await;

        let working_timestamp = chrono::Utc::now();
        if let Err(e) = self
            .repositories
            .tasks
            .update_task_state(&task.id, TaskState::Working, &working_timestamp)
            .await
        {
            return Err(AgentServiceError::operation(
                format!("Failed to mark task {} as working", task.id),
                e,
            ));
        }

        Ok(())
    }

    async fn persist_or_mark_failed(
        &self,
        task: &Task,
        user_message: &Message,
        agent_message: &Message,
        context: &RequestContext,
    ) -> Result<PersistOutcome> {
        let outcome = persist_completed_task(
            crate::services::a2a_server::processing::message::persistence::PersistCompletedTaskParams {
                task,
                user_message,
                agent_message,
                context,
                repositories: &self.repositories,
                publishing: &self.publishing,
                webhooks: self.webhooks(),
                artifacts_already_published: false,
            },
        )
        .await;
        let e = match outcome {
            Ok(outcome) => return Ok(outcome),
            Err(e) => e,
        };

        let error_msg = format!("Failed to persist completed task: {e}");
        tracing::error!(task_id = %task.id, error = %e, "Failed to persist completed task");

        let failed_timestamp = chrono::Utc::now();
        if let Err(update_err) = self
            .repositories
            .tasks
            .update_task_failed_with_error(&task.id, &error_msg, &failed_timestamp)
            .await
        {
            tracing::error!(task_id = %task.id, error = %update_err, "Failed to update task to failed state");
        }

        Err(e)
    }
}
