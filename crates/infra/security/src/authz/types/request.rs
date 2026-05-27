use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{Actor, McpToolName, ModelId, TraceId, UserId};

use super::decision::DenyReason;
use super::entity_ref::EntityRef;

/// Open extension point for the enforcement-site context attached to an
/// `AuthzRequest`. Replaces the previous closed enum so tenants can add
/// their own enforcement sites (skill execution, order submission, file
/// egress, ...) without a core change.
///
/// `kind` is a dotted-namespaced literal. Core mints three:
///
/// - `"none"` — no context (server-attach RBAC, etc).
/// - `"gateway.invocation"` — payload `{ "model": "..." }`.
/// - `"mcp.tool_call"` — payload `{ "tool": "..." }`.
///
/// Tenants mint their own (e.g. `"acme.order_submission"`) and recognise
/// them in their hook. Core never interprets `payload`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthzContext {
    pub kind: Cow<'static, str>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub payload: serde_json::Value,
}

impl Default for AuthzContext {
    fn default() -> Self {
        Self::none()
    }
}

impl AuthzContext {
    pub const NONE_KIND: &'static str = "none";
    pub const GATEWAY_INVOCATION_KIND: &'static str = "gateway.invocation";
    pub const MCP_TOOL_CALL_KIND: &'static str = "mcp.tool_call";

    #[must_use]
    pub const fn none() -> Self {
        Self {
            kind: Cow::Borrowed(Self::NONE_KIND),
            payload: serde_json::Value::Null,
        }
    }

    #[must_use]
    pub fn gateway_invocation(model: &ModelId) -> Self {
        Self {
            kind: Cow::Borrowed(Self::GATEWAY_INVOCATION_KIND),
            payload: serde_json::json!({ "model": model.as_str() }),
        }
    }

    #[must_use]
    pub fn mcp_tool_call(tool: &McpToolName) -> Self {
        Self {
            kind: Cow::Borrowed(Self::MCP_TOOL_CALL_KIND),
            payload: serde_json::json!({ "tool": tool.as_str() }),
        }
    }

    /// Tenant-facing constructor for an extension-defined enforcement site.
    /// `kind` should be dotted-namespaced (e.g. `"acme.order_submission"`).
    #[must_use]
    pub fn extension(kind: impl Into<Cow<'static, str>>, payload: serde_json::Value) -> Self {
        Self {
            kind: kind.into(),
            payload,
        }
    }

    #[must_use]
    pub fn gateway_invocation_model(&self) -> Option<ModelId> {
        if self.kind != Self::GATEWAY_INVOCATION_KIND {
            return None;
        }
        self.payload
            .get("model")
            .and_then(|v| v.as_str())
            .map(ModelId::new)
    }

    #[must_use]
    pub fn mcp_tool_call_tool(&self) -> Option<McpToolName> {
        if self.kind != Self::MCP_TOOL_CALL_KIND {
            return None;
        }
        self.payload
            .get("tool")
            .and_then(|v| v.as_str())
            .map(McpToolName::new)
    }

    #[must_use]
    pub fn is_none(&self) -> bool {
        self.kind == Self::NONE_KIND
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzRequest {
    pub entity: EntityRef,
    pub user_id: UserId,
    #[serde(default)]
    pub roles: Vec<String>,
    /// Opaque ABAC attribute bag forwarded from `JwtClaims.attributes`.
    /// Tenants namespace keys (e.g. `"acme.desk"`, `"boeing.clearance"`);
    /// core never interprets values.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
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
