mod message_handler;
mod persistence;
mod stream_processor;

pub use stream_processor::StreamProcessor;

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::a2a::{Artifact, Message, Task};
use crate::models::AgentRuntimeInfo;
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
use crate::repository::context::ContextRepository;
use crate::repository::execution::ExecutionStepRepository;
use crate::repository::task::TaskRepository;
use crate::services::{ContextService, SkillService};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::TaskId;
use systemprompt_models::RequestContext;

pub struct MessageProcessor {
    db_pool: DbPool,
    ai_service: Arc<dyn AiProvider>,
    task_repo: TaskRepository,
    context_repo: ContextRepository,
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
    pub fn new(db_pool: DbPool, ai_service: Arc<dyn AiProvider>) -> Result<Self> {
        let task_repo = TaskRepository::new(db_pool.clone());
        let context_repo = ContextRepository::new(db_pool.clone());
        let context_service = ContextService::new(db_pool.clone());
        let skill_service = Arc::new(SkillService::new(db_pool.clone()));
        let execution_step_repo = Arc::new(ExecutionStepRepository::new(db_pool.clone())?);

        Ok(Self {
            db_pool,
            ai_service,
            task_repo,
            context_repo,
            context_service,
            skill_service,
            execution_step_repo,
        })
    }

    pub async fn load_agent_runtime(&self, agent_name: &str) -> Result<AgentRuntimeInfo> {
        use crate::services::registry::AgentRegistry;

        let registry = AgentRegistry::new().await?;
        let agent_config = registry
            .get_agent(agent_name)
            .await
            .map_err(|_| anyhow!("Agent not found"))?;

        Ok(agent_config.into())
    }

    pub async fn persist_completed_task(
        &self,
        task: &Task,
        user_message: &Message,
        agent_message: &Message,
        context: &RequestContext,
        _agent_name: &str,
        artifacts_already_published: bool,
    ) -> Result<Task> {
        persistence::persist_completed_task(
            task,
            user_message,
            agent_message,
            context,
            &self.task_repo,
            &self.db_pool,
            artifacts_already_published,
        )
        .await
    }

    pub async fn process_message_stream(
        &self,
        a2a_message: &Message,
        agent_runtime: &AgentRuntimeInfo,
        agent_name: &str,
        context: &RequestContext,
        task_id: TaskId,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent>> {
        let stream_processor = StreamProcessor {
            ai_service: self.ai_service.clone(),
            context_service: self.context_service.clone(),
            skill_service: self.skill_service.clone(),
            execution_step_repo: self.execution_step_repo.clone(),
        };

        stream_processor
            .process_message_stream(a2a_message, agent_runtime, agent_name, context, task_id)
            .await
    }
}
