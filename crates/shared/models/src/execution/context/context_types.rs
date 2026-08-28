//! Context-related type definitions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub jti: String,
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
            session_id: SessionId::new("unknown".to_owned()),
            timestamp: Instant::now(),
            client_id: None,
            is_tracked: true,
            fingerprint_hash: None,
        }
    }
}

/// Per-request execution facts propagated across service hops.
///
/// `agent_name` is the platform agent handling this run — an A2A service's
/// own name once the request reaches it. It is `AgentName::system()` when no
/// platform agent is involved (a human, bridge, or direct API caller), which
/// is a defined value and not a stand-in for "unknown".
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
