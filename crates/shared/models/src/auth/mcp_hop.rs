//! The proxy-to-MCP-server hop token.
//!
//! The API proxy authenticates every request at the edge, then forwards it to
//! the managed MCP server process on loopback with a short-lived RS256 token
//! it signs with the deployment key. The token's audience is
//! [`mcp_hop_audience`] for exactly one server, its JOSE `typ` is
//! [`MCP_HOP_TOKEN_TYPE`] (no other signer in the deployment sets it), and its
//! claims are the caller's verified identity and the execution it is
//! attributed to. The MCP server builds its authenticated context and its
//! execution attribution from these claims and from nothing else the request
//! carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Duration;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ClientId, ContextId, McpServerId, SessionId, TaskId, TraceId,
    UserId,
};

use super::{AuthenticatedUser, JwtAudience, RegisteredClaims, TokenInteraction, UserType};
use crate::execution::context::{CallSource, RequestContext, UserInteractionMode};

pub const MCP_HOP_TOKEN_TYPE: &str = "mcp-hop+jwt";

pub const MCP_HOP_TTL: Duration = Duration::seconds(60);

pub const MCP_HOP_MAX_LIFETIME: Duration = Duration::minutes(5);

#[must_use]
pub fn mcp_hop_audience(server: &McpServerId) -> JwtAudience {
    JwtAudience::Resource(format!("mcp:{}", server.as_str()))
}

/// The verified caller a hop token vouches for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHopIdentity {
    pub user_id: UserId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<AuthenticatedUser>,
    pub session_id: SessionId,
    pub user_type: UserType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<TokenInteraction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<McpHopExecution>,
}

/// The execution a hop token was minted for.
///
/// The conversation, trace, task, tool call and agent the edge attributed the
/// request to. Every core sink
/// that records an execution files it under these values, so they are signed
/// with the identity rather than read from headers. A token without one (a
/// health probe) vouches for no execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpHopExecution {
    pub context_id: ContextId,
    pub trace_id: TraceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub agent_name: AgentName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_source: Option<CallSource>,
}

impl McpHopExecution {
    #[must_use]
    pub fn of(context: &RequestContext) -> Self {
        Self {
            context_id: context.execution.context_id.clone(),
            trace_id: context.execution.trace_id.clone(),
            task_id: context.execution.task_id.clone(),
            ai_tool_call_id: context.execution.ai_tool_call_id.clone(),
            agent_name: context.execution.agent_name.clone(),
            call_source: context.execution.call_source,
        }
    }

    fn bind(&self, context: &mut RequestContext) {
        context.execution.context_id = self.context_id.clone();
        context.execution.trace_id = self.trace_id.clone();
        context.execution.task_id.clone_from(&self.task_id);
        context
            .execution
            .ai_tool_call_id
            .clone_from(&self.ai_tool_call_id);
        context.execution.agent_name = self.agent_name.clone();
        context.execution.call_source = self.call_source;
    }
}

impl McpHopIdentity {
    #[must_use]
    pub fn bind(&self, context: RequestContext) -> RequestContext {
        let mut context = context;
        if let Some(execution) = &self.execution {
            execution.bind(&mut context);
        }
        context.request.session_id = self.session_id.clone();
        context.request.client_id.clone_from(&self.client_id);
        let mut context = context
            .with_actor(self.actor())
            .with_user_type(self.user_type)
            .with_act_chain(self.act_chain.clone());
        if let Some(user) = self.user.clone() {
            context = context.with_user(user);
        }
        if let Some(purpose) = self.purpose.clone() {
            context = context.with_caller_purpose(purpose);
        }
        if let Some(mode) = self.interaction_mode() {
            context = context.with_interaction_mode(mode);
        }
        context
    }

    #[must_use]
    pub fn vouches_for(&self, context: &RequestContext) -> bool {
        context.auth.actor == self.actor()
            && context.auth.user_type == self.user_type
            && context.auth.act_chain == self.act_chain
            && context.request.session_id == self.session_id
            && context.request.client_id == self.client_id
            && context.user == self.user
            && self
                .execution
                .as_ref()
                .is_some_and(|execution| *execution == McpHopExecution::of(context))
            && self
                .purpose
                .as_ref()
                .is_none_or(|purpose| context.execution.caller_purpose.as_ref() == Some(purpose))
            && self
                .interaction_mode()
                .is_none_or(|mode| context.settings.user_interaction_mode == Some(mode))
    }

    fn interaction_mode(&self) -> Option<UserInteractionMode> {
        self.interaction.map(|interaction| match interaction {
            TokenInteraction::Interactive => UserInteractionMode::Interactive,
            TokenInteraction::NonInteractive => UserInteractionMode::NonInteractive,
        })
    }

    fn actor(&self) -> Actor {
        self.user.as_ref().map_or_else(
            || Actor::anonymous(self.user_id.clone()),
            |user| Actor::user(user.id.clone()),
        )
    }
}

/// Wire claims of a hop token: the registered claims and the identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHopClaims {
    #[serde(flatten)]
    pub registered: RegisteredClaims,
    #[serde(flatten)]
    pub identity: McpHopIdentity,
}
