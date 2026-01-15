//! Context-related type definitions.

use crate::ai::ToolModelConfig;
use crate::auth::UserType;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use systemprompt_identifiers::{
    AgentName, AiToolCallId, ClientId, ContextId, JwtToken, McpExecutionId, SessionId, TaskId,
    TraceId, UserId,
};

use super::CallSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub auth_token: JwtToken,
    pub user_id: UserId,
    pub user_type: UserType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub session_id: SessionId,
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    pub client_id: Option<ClientId>,
    pub is_tracked: bool,
}

impl Default for RequestMetadata {
    fn default() -> Self {
        Self {
            session_id: SessionId::new("unknown".to_string()),
            timestamp: Instant::now(),
            client_id: None,
            is_tracked: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub trace_id: TraceId,
    pub context_id: ContextId,
    pub task_id: Option<TaskId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub call_source: Option<CallSource>,
    pub agent_name: AgentName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_model_config: Option<ToolModelConfig>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            trace_id: TraceId::new(uuid::Uuid::new_v4().to_string()),
            context_id: ContextId::new(String::new()),
            task_id: None,
            ai_tool_call_id: None,
            mcp_execution_id: None,
            call_source: None,
            agent_name: AgentName::system(),
            tool_model_config: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ExecutionSettings {
    pub max_budget_cents: Option<i32>,
    pub user_interaction_mode: Option<UserInteractionMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserInteractionMode {
    Interactive,
    NonInteractive,
}
