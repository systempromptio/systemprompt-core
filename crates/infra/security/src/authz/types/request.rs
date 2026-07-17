//! `AuthzRequest` and the open enforcement-site `AuthzContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    Actor, ContextId, McpToolName, ModelId, SessionId, TaskId, TraceId, UserId,
};

use super::decision::DenyReason;
use super::entity_ref::EntityRef;

/// Open enforcement-site context attached to an [`AuthzRequest`].
///
/// Replaces the previous closed enum so tenants can add their own
/// enforcement sites (skill execution, order submission, file egress, ...)
/// without a core change.
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

    /// `kind` must be dotted-namespaced (e.g. `"acme.order_submission"`) so
    /// kinds from independent extensions cannot collide.
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

    pub const MARKETPLACE_FLOOR_KEY: &'static str = "marketplace.attribute_floor";

    /// The floor is an opaque tenant-namespaced bag the ABAC hook interprets;
    /// core copies it verbatim. Keyed under [`MARKETPLACE_FLOOR_KEY`] so it
    /// never collides with the typed `model` / `tool` payload entries, and
    /// `kind` plus any existing payload are preserved.
    ///
    /// [`MARKETPLACE_FLOOR_KEY`]: Self::MARKETPLACE_FLOOR_KEY
    #[must_use]
    pub fn with_marketplace_floor(&self, floor: &BTreeMap<String, serde_json::Value>) -> Self {
        let mut payload = match self.payload.clone() {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let floor_value = floor
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        payload.insert(
            Self::MARKETPLACE_FLOOR_KEY.to_owned(),
            serde_json::Value::Object(floor_value),
        );
        Self {
            kind: self.kind.clone(),
            payload: serde_json::Value::Object(payload),
        }
    }

    #[must_use]
    pub fn marketplace_floor(&self) -> Option<BTreeMap<String, serde_json::Value>> {
        let obj = self.payload.get(Self::MARKETPLACE_FLOOR_KEY)?.as_object()?;
        Some(
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<String, serde_json::Value>>(),
        )
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
    /// Attested session this authorization request was made under, when the
    /// enforcement site has one (gateway path). Threaded into the audit row's
    /// `session_id` column; non-session paths (server-attach RBAC, MCP) leave
    /// it `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default)]
    pub context: AuthzContext,
    /// A2A conversation the enforcement site is acting within, when it has
    /// one (gateway-derived conversation, MCP tool-call execution context,
    /// messaging). Threaded into the audit row's `context_id` column so agent
    /// conversations reconstruct by key instead of user+time-window joins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
    /// A2A task the enforcement site is acting within, when one exists
    /// (internal agent tool-calls). Threaded into the audit row's `task_id`
    /// column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
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
