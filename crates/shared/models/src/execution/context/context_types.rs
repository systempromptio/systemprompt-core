//! Context-related type definitions.

use crate::ai::ToolModelConfig;
use crate::auth::UserType;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ClientId, ContextId, JwtToken, McpExecutionId, SessionId,
    TaskId, TraceId,
};

use super::CallSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub auth_token: JwtToken,
    pub actor: Actor,
    pub user_type: UserType,
    /// RFC 8693 actor (`act`) chain in outermost-first order: index 0 is
    /// the most recent delegate that requested the current token, and the
    /// last entry is the original delegating principal. Empty for direct
    /// (non-delegated) tokens.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
    /// JWT `jti`. Empty for anonymous / system contexts. Carried forward so
    /// the JTI-revocation tower layer can consult the revocation list without
    /// re-decoding the bearer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub jti: String,
    /// JWT `exp` (unix seconds). Zero for anonymous / system contexts.
    /// Required by `POST /oauth/logout` to write the revocation row with the
    /// token's natural lifetime.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub token_exp: i64,
}

const fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub session_id: SessionId,
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    pub client_id: Option<ClientId>,
    pub is_tracked: bool,
    pub fingerprint_hash: Option<String>,
}

impl Default for RequestMetadata {
    fn default() -> Self {
        Self {
            session_id: SessionId::new("unknown".to_string()),
            timestamp: Instant::now(),
            client_id: None,
            is_tracked: true,
            fingerprint_hash: None,
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
            context_id: ContextId::generate(),
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
