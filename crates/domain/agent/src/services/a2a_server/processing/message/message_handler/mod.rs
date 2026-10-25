//! Non-streaming message handling for [`MessageHandler`].
//!
//! Implements `MessageHandler::handle_message`: it validates the context,
//! persists a submitted task, runs the stream pipeline to completion, builds
//! the finished [`Task`], persists it, and broadcasts
//! the completion and AG-UI lifecycle events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod lifecycle;
mod shapes;
mod stream_collection;

pub use shapes::{HandleMessageParams, new_submitted_task, resolve_agent_message, resolve_task_id};

use std::sync::Arc;

use crate::services::a2a_server::active_tasks::ActiveTasks;

use self::lifecycle::{FinalizeParams, PersistAndAnnounceParams};
use self::stream_collection::collect_stream_response;
use crate::models::a2a::{Artifact, Message, Task, TaskState};
use crate::services::a2a_server::processing::message::stream_service::StreamService;
use crate::services::a2a_server::processing::message::{
    MessageHandler, ProcessMessageStreamParams,
};
use crate::services::a2a_server::processing::task_builder::build_completed_task;
use crate::services::a2a_server::streaming::webhook_client::WebhookContext;
use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::{AgentName, ContextId, TaskId};
use systemprompt_models::RequestContext;

impl MessageHandler {
    pub(in crate::services::a2a_server) async fn handle_message(
        &self,
        message: Message,
        agent_name: &AgentName,
        context: &RequestContext,
        active_tasks: &ActiveTasks,
    ) -> Result<Task> {
        let agent_runtime = self.load_agent_runtime(agent_name)?;
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

        let (response_text, tool_artifacts) = self
            .run_to_completion(
                ProcessMessageStreamParams {
                    a2a_message: &message,
                    agent_runtime,
                    agent_name,
                    context,
                    task_id: task_id.clone(),
                    cancel: guard.token(),
                },
                &webhooks,
            )
            .await?;

        let task = build_completed_task(
            task_id,
            context_id.clone(),
            response_text.clone(),
            message.clone(),
            tool_artifacts,
        );

        let agent_message = resolve_agent_message(&task, &message, &response_text);
        self.finalize(FinalizeParams {
            task: &task,
            message: &message,
            agent_message: &agent_message,
            response_text: &response_text,
            context,
            webhooks: &webhooks,
        })
        .await?;

        Ok(task)
    }

    async fn run_to_completion(
        &self,
        params: ProcessMessageStreamParams<'_>,
        webhooks: &WebhookContext,
    ) -> Result<(String, Vec<Artifact>)> {
        let task_id = params.task_id.clone();
        let stream = self.stream_service().process_message_stream(params).await?;
        match collect_stream_response(stream, webhooks).await {
            Err(AgentServiceError::TaskCancelled) => {
                self.mark_cancelled(&task_id).await?;
                Err(AgentServiceError::TaskCancelled)
            },
            collected => collected,
        }
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
                AgentServiceError::operation(
                    format!(
                        "Context validation failed - context_id: {context_id}, user_id: {}",
                        context.user_id()
                    ),
                    e,
                )
            })?;

        tracing::info!(
            context_id = %context_id,
            user_id = %context.user_id(),
            "Context validated"
        );
        Ok(())
    }

    fn stream_service(&self) -> StreamService {
        StreamService {
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
                AgentServiceError::operation(format!("Failed to mark task {task_id} cancelled"), e)
            })
    }
}
