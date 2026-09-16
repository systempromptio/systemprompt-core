//! Message processing for the A2A server.
//!
//! [`MessageProcessor`] owns the repositories and services needed to handle an
//! inbound message and persist the resulting task. [`StreamProcessor`] drives
//! the streaming execution pipeline, reporting progress as [`StreamEvent`]s
//! over an mpsc channel. Both the streaming and non-streaming entry points live
//! in the submodules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod content;
pub mod message_handler;
pub mod persistence;
pub mod stream_processor;

pub use content::extract_message_content;
pub use message_handler::HandleMessageParams;
pub use persistence::PersistOutcome;
pub use stream_processor::StreamProcessor;

use crate::services::shared::{AgentServiceError, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::models::AgentRuntimeInfo;
use crate::models::a2a::{Artifact, Message, Task};
use crate::repository::A2ARepositories;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::a2a_server::streaming::webhook_client::DynWebhookBroadcaster;
use crate::services::{ArtifactPublishingService, ContextService, SkillService};
use systemprompt_identifiers::{AiToolCallId, TaskId};
use systemprompt_models::{AiProvider, CallToolResult, RequestContext, ToolCall};

#[derive(Debug)]
pub enum StreamEvent {
    Text(String),
    ToolCallStarted(ToolCall),
    ToolResult {
        ai_tool_call_id: AiToolCallId,
        result: CallToolResult,
    },
    ExecutionStepUpdate {
        step: crate::models::ExecutionStep,
    },
    Complete {
        full_text: String,
        artifacts: Vec<Artifact>,
    },
    Error(String),
    Cancelled,
}

/// A running message pipeline: its event receiver plus the owned worker and
/// the token that stops it. Dropping the stream aborts the worker.
pub struct MessageStream {
    pub events: mpsc::Receiver<StreamEvent>,
    pub worker: tokio::task::JoinHandle<()>,
    pub cancel: CancellationToken,
}

impl std::fmt::Debug for MessageStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageStream")
            .field("cancelled", &self.cancel.is_cancelled())
            .field("worker_finished", &self.worker.is_finished())
            .finish_non_exhaustive()
    }
}

impl Drop for MessageStream {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

#[derive(Debug)]
pub struct PersistCompletedTaskOnProcessorParams<'a> {
    pub task: &'a Task,
    pub user_message: &'a Message,
    pub agent_message: &'a Message,
    pub context: &'a RequestContext,
    pub agent_name: &'a str,
    pub artifacts_already_published: bool,
}

#[derive(Debug)]
pub struct ProcessMessageStreamParams<'a> {
    pub a2a_message: &'a Message,
    pub agent_runtime: &'a AgentRuntimeInfo,
    pub agent_name: &'a str,
    pub context: &'a RequestContext,
    pub task_id: TaskId,
    pub cancel: CancellationToken,
}

pub struct MessageProcessor {
    repositories: Arc<A2ARepositories>,
    ai_service: Arc<dyn AiProvider>,
    context_service: ContextService,
    skill_service: Arc<SkillService>,
    execution_step_repo: Arc<ExecutionStepRepository>,
    publishing: ArtifactPublishingService,
    webhooks: DynWebhookBroadcaster,
}

impl std::fmt::Debug for MessageProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageProcessor")
            .field("ai_service", &"<Arc<dyn AiProvider>>")
            .finish()
    }
}

impl MessageProcessor {
    pub fn new(
        repositories: Arc<A2ARepositories>,
        ai_service: Arc<dyn AiProvider>,
        webhooks: DynWebhookBroadcaster,
    ) -> Result<Self> {
        let context_service = ContextService::new(repositories.tasks.clone());
        let execution_step_repo = Arc::new(repositories.execution_steps.clone());
        let skill_service = Arc::new(SkillService::new(
            repositories.managed_skill_resolver(),
            Arc::clone(&execution_step_repo),
            Arc::clone(&webhooks),
        )?);
        let publishing = ArtifactPublishingService::new(&repositories, Arc::clone(&skill_service));

        Ok(Self {
            repositories,
            ai_service,
            context_service,
            skill_service,
            execution_step_repo,
            publishing,
            webhooks,
        })
    }

    #[must_use]
    pub fn webhooks(&self) -> DynWebhookBroadcaster {
        Arc::clone(&self.webhooks)
    }

    pub async fn load_agent_runtime(&self, agent_name: &str) -> Result<AgentRuntimeInfo> {
        use crate::services::registry::AgentRegistry;

        let registry = AgentRegistry::new()?;
        let agent_config = registry
            .get_agent(agent_name)
            .await
            .map_err(|_e| AgentServiceError::Internal("Agent not found".to_owned()))?;

        Ok(agent_config.into())
    }

    pub async fn persist_completed_task(
        &self,
        params: PersistCompletedTaskOnProcessorParams<'_>,
    ) -> Result<PersistOutcome> {
        persistence::persist_completed_task(persistence::PersistCompletedTaskParams {
            task: params.task,
            user_message: params.user_message,
            agent_message: params.agent_message,
            context: params.context,
            repositories: &self.repositories,
            publishing: &self.publishing,
            webhooks: Arc::clone(&self.webhooks),
            artifacts_already_published: params.artifacts_already_published,
        })
        .await
    }

    pub async fn process_message_stream(
        &self,
        params: ProcessMessageStreamParams<'_>,
    ) -> Result<MessageStream> {
        let stream_processor = StreamProcessor {
            ai_service: Arc::clone(&self.ai_service),
            context_service: self.context_service.clone(),
            skill_service: Arc::clone(&self.skill_service),
            execution_step_repo: Arc::clone(&self.execution_step_repo),
        };

        stream_processor.process_message_stream(params).await
    }
}
