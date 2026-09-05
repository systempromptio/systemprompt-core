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

mod message_handler;
pub(crate) mod persistence;
pub(crate) mod stream_processor;

pub use stream_processor::StreamProcessor;

use crate::services::shared::{AgentServiceError, Result};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::AgentRuntimeInfo;
use crate::models::a2a::{Artifact, Message, Task};
use systemprompt_models::{AiProvider, CallToolResult, ToolCall};

#[derive(Debug)]
pub enum StreamEvent {
    Text(String),
    ToolCallStarted(ToolCall),
    ToolResult {
        call_id: String,
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
}
use crate::repository::A2ARepositories;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::{ContextService, SkillService};
use systemprompt_identifiers::TaskId;
use systemprompt_models::RequestContext;

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
}

pub struct MessageProcessor {
    repositories: Arc<A2ARepositories>,
    ai_service: Arc<dyn AiProvider>,
    context_service: ContextService,
    skill_service: Arc<SkillService>,
    execution_step_repo: Arc<ExecutionStepRepository>,
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
    ) -> Result<Self> {
        let context_service = ContextService::new(repositories.tasks.clone());
        let execution_step_repo = Arc::new(repositories.execution_steps.clone());
        let skill_service = Arc::new(
            SkillService::new()?.with_execution_step_repo(Arc::clone(&execution_step_repo)),
        );

        Ok(Self {
            repositories,
            ai_service,
            context_service,
            skill_service,
            execution_step_repo,
        })
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
    ) -> Result<Task> {
        persistence::persist_completed_task(persistence::PersistCompletedTaskParams {
            task: params.task,
            user_message: params.user_message,
            agent_message: params.agent_message,
            context: params.context,
            repositories: &self.repositories,
            artifacts_already_published: params.artifacts_already_published,
        })
        .await
    }

    pub async fn process_message_stream(
        &self,
        params: ProcessMessageStreamParams<'_>,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let stream_processor = StreamProcessor {
            ai_service: Arc::clone(&self.ai_service),
            context_service: self.context_service.clone(),
            skill_service: Arc::clone(&self.skill_service),
            execution_step_repo: Arc::clone(&self.execution_step_repo),
        };

        stream_processor.process_message_stream(params).await
    }
}
