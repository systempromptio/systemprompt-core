//! Non-streaming message handling for [`MessageProcessor`].
//!
//! Implements `MessageProcessor::handle_message`: it validates the context,
//! persists a submitted task, runs the stream pipeline to completion, builds
//! the finished [`Task`], persists it, and broadcasts
//! the completion and AG-UI lifecycle events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod helpers;
mod shapes;

pub use shapes::{HandleMessageParams, new_submitted_task, resolve_agent_message, resolve_task_id};

use std::sync::Arc;

use crate::services::a2a_server::active_tasks::ActiveTasks;

use self::helpers::{
    BroadcastAguiLifecycleParams, broadcast_agui_lifecycle, collect_stream_response,
};
use crate::models::a2a::{Message, Task, TaskState};
use crate::services::a2a_server::processing::message::persistence::{
    PersistOutcome, broadcast_completion, persist_completed_task,
};
use crate::services::a2a_server::processing::message::stream_processor::StreamProcessor;
use crate::services::a2a_server::processing::message::{
    MessageProcessor, ProcessMessageStreamParams,
};
use crate::services::a2a_server::processing::task_builder::build_completed_task;
use crate::services::a2a_server::streaming::broadcast::{
    BroadcastTaskCreatedParams, broadcast_task_created,
};
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;
use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::RequestContext;

struct PersistAndAnnounceParams<'a> {
    task: &'a Task,
    message: &'a Message,
    agent_name: &'a str,
    context: &'a RequestContext,
    webhooks: &'a WebhookContext,
}

impl MessageProcessor {
    pub(in crate::services::a2a_server) async fn handle_message(
        &self,
        message: Message,
        agent_name: &str,
        context: &RequestContext,
        active_tasks: &ActiveTasks,
    ) -> Result<Task> {
        let agent_runtime = self.load_agent_runtime(agent_name).await?;
        self.handle_message_with_runtime(HandleMessageParams {
            message,
            agent_runtime: &agent_runtime,
            agent_name,
            context,
            active_tasks,
        })
        .await
    }

    pub async fn handle_message_with_runtime(
        &self,
        params: HandleMessageParams<'_>,
    ) -> Result<Task> {
        let HandleMessageParams {
            message,
            agent_runtime,
            agent_name,
            context,
            active_tasks,
        } = params;
        tracing::info!(agent_name = %agent_name, "Handling non-streaming message");

        let webhooks = WebhookContext::for_request(self.webhooks(), context);
        let context_id = &message.context_id;
        self.validate_context(context_id, context).await?;

        let task_id = resolve_task_id(&message);
        let guard = active_tasks.register(task_id.clone());
        let task = new_submitted_task(&task_id, context_id, agent_name);

        self.persist_and_announce(PersistAndAnnounceParams {
            task: &task,
            message: &message,
            agent_name,
            context,
            webhooks: &webhooks,
        })
        .await?;

        let stream = self
            .stream_processor()
            .process_message_stream(ProcessMessageStreamParams {
                a2a_message: &message,
                agent_runtime,
                agent_name,
                context,
                task_id: task_id.clone(),
                cancel: guard.token(),
            })
            .await?;

        let (response_text, tool_artifacts) = match collect_stream_response(stream, &webhooks).await
        {
            Ok(collected) => collected,
            Err(AgentServiceError::TaskCancelled) => {
                self.mark_cancelled(&task_id).await?;
                return Err(AgentServiceError::TaskCancelled);
            },
            Err(e) => return Err(e),
        };

        let task = build_completed_task(
            task_id,
            context_id.clone(),
            response_text.clone(),
            message.clone(),
            tool_artifacts,
        );

        let agent_message = resolve_agent_message(&task, &message, &response_text);

        if context.user_type() == systemprompt_models::auth::UserType::Anon {
            tracing::warn!(
                context_id = %context_id,
                session_id = %context.session_id(),
                "Saving messages for anonymous user"
            );
        }

        self.persist_or_mark_failed(&task, &message, &agent_message, context)
            .await?
            .record_undelivered_broadcasts();

        broadcast_completion(self.webhooks(), &task, context).await;

        broadcast_agui_lifecycle(BroadcastAguiLifecycleParams {
            webhooks: &webhooks,
            context_id,
            task: &task,
            agent_message: &agent_message,
            response_text: &response_text,
        })
        .await;

        Ok(task)
    }

    async fn validate_context(
        &self,
        context_id: &ContextId,
        context: &RequestContext,
    ) -> Result<()> {
        self.repositories
            .contexts
            .get_context(context_id, context.user_id())
            .await
            .map_err(|e| {
                AgentServiceError::Internal(format!(
                    "Context validation failed - context_id: {context_id}, user_id: {}, error: {e}",
                    context.user_id()
                ))
            })?;

        tracing::info!(
            context_id = %context_id,
            user_id = %context.user_id(),
            "Context validated"
        );
        Ok(())
    }

    fn stream_processor(&self) -> StreamProcessor {
        StreamProcessor {
            ai_service: Arc::clone(&self.ai_service),
            context_service: self.context_service.clone(),
            skill_service: Arc::clone(&self.skill_service),
            execution_step_repo: Arc::clone(&self.execution_step_repo),
        }
    }

    async fn mark_cancelled(&self, task_id: &TaskId) -> Result<()> {
        self.repositories
            .tasks
            .update_task_state(task_id, TaskState::Canceled, &chrono::Utc::now())
            .await
            .map_err(|e| {
                AgentServiceError::Internal(format!("Failed to mark task {task_id} cancelled: {e}"))
            })
    }

    async fn persist_and_announce(&self, params: PersistAndAnnounceParams<'_>) -> Result<()> {
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
            return Err(AgentServiceError::Internal(format!(
                "Failed to persist task at start: {e}"
            )));
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
            return Err(AgentServiceError::Internal(format!(
                "Failed to mark task {} as working: {e}",
                task.id
            )));
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
