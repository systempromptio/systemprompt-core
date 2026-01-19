use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::AgentName;
use systemprompt_models::{
    AiMessage, AiProvider, CallToolResult, ContextId, McpTool, RequestContext, TaskId, ToolCall,
};
use tokio::sync::mpsc;

use super::message::StreamEvent;
use crate::models::AgentRuntimeInfo;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::SkillService;

#[derive(Clone)]
pub struct ExecutionContext {
    pub ai_service: Arc<dyn AiProvider>,
    pub skill_service: Arc<SkillService>,
    pub agent_runtime: AgentRuntimeInfo,
    pub agent_name: AgentName,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub tx: mpsc::UnboundedSender<StreamEvent>,
    pub request_ctx: RequestContext,
    pub execution_step_repo: Arc<ExecutionStepRepository>,
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("agent_name", &self.agent_name)
            .field("task_id", &self.task_id)
            .field("context_id", &self.context_id)
            .finish()
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub accumulated_text: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<CallToolResult>,
    pub tools: Vec<McpTool>,
    pub iterations: usize,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            accumulated_text: String::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            tools: Vec::new(),
            iterations: 1,
        }
    }
}

#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    async fn execute(
        &self,
        context: ExecutionContext,
        messages: Vec<AiMessage>,
    ) -> Result<ExecutionResult>;

    fn name(&self) -> &'static str;
}

pub mod plan_executor;
pub mod planned;
pub mod selector;
pub mod standard;
pub mod tool_executor;

pub use plan_executor::{
    convert_to_call_tool_results, convert_to_tool_calls, execute_tools_sequentially,
    execute_tools_with_templates, format_results_for_response, ToolExecutorTrait,
};
pub use planned::PlannedAgenticStrategy;
pub use selector::ExecutionStrategySelector;
pub use standard::StandardExecutionStrategy;
pub use tool_executor::ContextToolExecutor;
