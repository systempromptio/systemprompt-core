//! `AuthzRequest` and the open enforcement-site `AuthzContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    Actor, ActorKind, ClientId, ContextId, McpToolName, ModelId, SessionId, TaskId, TraceId, UserId,
};

use super::decision::DenyReason;
use super::entity_ref::EntityRef;
use crate::policy::types::AccessScope;

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

/// One authorization question, as sent to the configured hook.
///
/// This struct crosses the wire as JSON to an out-of-process hook, so every
/// field added after `user_id` is optional on the wire: a hook built against
/// an older shape must still parse a newer request, or every governed call
/// would fail closed while the two sides are deployed separately.
///
/// `actor` is the surface the request came through (user, mcp server, agent,
/// job); its `user_id` MUST equal the top-level `user_id`. Build through
/// [`AuthzRequest::for_actor`] so the two cannot diverge. `client_id` is the
/// OAuth client from the validated token, never from a request header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzRequest {
    pub entity: EntityRef,
    pub user_id: UserId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_scope: Option<AccessScope>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub trace_id: TraceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default)]
    pub context: AuthzContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
}

impl AuthzRequest {
    #[must_use]
    pub fn for_actor(mut self, actor: Actor) -> Self {
        self.user_id = actor.user_id.clone();
        self.actor = Some(actor);
        self
    }

    #[must_use]
    pub fn actor(&self) -> Actor {
        self.actor
            .clone()
            .unwrap_or_else(|| Actor::user(self.user_id.clone()))
    }

    // Why: the direct caller is the outermost `act` link -- the most recent
    // delegate -- and only a delegate that is itself an agent is a verified
    // agent identity. A chain of plain users yields no agent, which is honest.
    #[must_use]
    pub fn verified_agent_id(&self) -> Option<&str> {
        match self.act_chain.first().map(|a| &a.kind) {
            Some(ActorKind::Agent { agent_id }) => Some(agent_id.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum AuthzDecision {
    Allow,
    Deny { reason: DenyReason, policy: String },
}
