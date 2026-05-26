use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{Actor, McpToolName, ModelId, TraceId, UserId};

use super::decision::DenyReason;
use super::entity_ref::EntityRef;

/// Typed per-request context forwarded to the authz hook. The variant is
/// the type of enforcement site that fired; downstream policy handlers
/// pattern-match instead of poking at `serde_json::Value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthzContext {
    /// Gateway `/v1/messages` invocation — carries the model literal the
    /// client requested (the route in `entity` already encodes routing
    /// policy, but the literal is needed for audit and downstream rules).
    GatewayInvocation { model: ModelId },
    /// MCP tool call about to be dispatched.
    McpToolCall { tool: McpToolName },
    /// No context (RBAC server-attach checks, etc).
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzRequest {
    pub entity: EntityRef,
    pub user_id: UserId,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub department: String,
    pub trace_id: TraceId,
    #[serde(default)]
    pub context: AuthzContext,
    /// RFC 8693 delegation lineage forwarded from
    /// `RequestContext.auth.act_chain`. Empty when no token-exchange chain
    /// is present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum AuthzDecision {
    Allow,
    Deny { reason: DenyReason, policy: String },
}
