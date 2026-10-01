//! Platform-agnostic dispatch for chat-platform inbound messages.
//!
//! Slack and Teams differ only at their edges — request verification, payload
//! shape, and reply rendering. Everything between (identity, authorization,
//! deterministic conversation context, per-user A2A token minting, the blocking
//! `message/send` through the proxy, and reply extraction) is identical and
//! lives here once. A per-platform route normalizes its wire payload into a
//! [`MessagingInbound`] and calls [`dispatch_messaging`]; the returned
//! [`DispatchOutcome`] is rendered back into the platform's UI by the route.
//!
//! The pipeline is **synchronous, spawned**: the route acks the platform within
//! its timeout, then a spawned task runs this blocking dispatch and posts the
//! reply. There is no responder job and no dispatch-state table — a stable
//! [`ContextId`](systemprompt_identifiers::ContextId) (derived from the
//! conversation) ties multi-turn history together instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod a2a;
pub mod conversation;
pub mod identity;

use std::sync::LazyLock;

use serde_json::json;
use systemprompt_identifiers::{Actor, AgentName, SessionId, TraceId};
use systemprompt_oauth::OauthError;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::{AuthzContext, AuthzDecision, AuthzRequest, EntityRef};
use systemprompt_traits::SenderIdentity;
use systemprompt_users::UserError;

use crate::services::proxy::ProxyError;
use a2a::{authenticated_user, build_a2a_request, mint_a2a_token, run_agent};
pub use conversation::MessagingConversation;
use identity::resolve_or_link_user;

static GUARDED_CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    systemprompt_client::guarded_client(&systemprompt_client::GuardedClientConfig::default())
        .inspect_err(|e| tracing::error!(error = %e, "Guarded outbound http client unavailable"))
        .ok()
});

// Why: Slack replies target a caller-supplied `response_url`, so they must go
// through the connect-time SSRF guard rather than the plain client the
// operator-configured Teams endpoints use.
#[must_use]
pub fn guarded_http_client() -> Option<reqwest::Client> {
    GUARDED_CLIENT.clone()
}

#[derive(Debug, Clone)]
pub enum ReplyTarget {
    Channel { id: String },
    Url { url: String },
}

/// A surface-agnostic inbound message ready for dispatch. Per-platform routes
/// build this from their normalized payload; the dispatch core never sees a
/// Slack- or Teams-specific type.
#[derive(Debug, Clone)]
pub struct MessagingInbound {
    pub issuer: String,
    pub conversation: MessagingConversation,
    pub text: String,
    pub agent_name: AgentName,
    pub entity: EntityRef,
    pub reply: ReplyTarget,
    pub sender: SenderIdentity,
}

#[derive(Debug, Clone)]
pub enum DispatchOutcome {
    Replied(String),
    Denied(String),
}

/// Failures along the dispatch pipeline. This is an internal system surface;
/// messages are deliberately descriptive for operator debugging.
#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("identity resolution failed")]
    Identity(#[source] UserError),
    #[error("token minting failed")]
    Token(#[source] OauthError),
    #[error("could not encode the agent request")]
    Encode(#[source] serde_json::Error),
    #[error("could not build the agent request")]
    Request(#[source] http::Error),
    #[error("agent dispatch failed")]
    Dispatch(#[source] ProxyError),
    #[error("agent returned JSON-RPC error {code}: {message}")]
    AgentRejected { code: i32, message: String },
    #[error("agent response body could not be read")]
    ResponseBody(#[source] axum::Error),
    #[error("malformed agent response")]
    Response(#[source] serde_json::Error),
}

impl MessagingError {
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "opaque by contract: no part of the error may reach the caller"
    )]
    pub fn user_message(&self) -> String {
        "Sorry — something went wrong handling that.".to_owned()
    }
}

pub async fn dispatch_messaging(
    ctx: &AppContext,
    inbound: MessagingInbound,
) -> Result<DispatchOutcome, MessagingError> {
    let user = resolve_or_link_user(
        ctx,
        &inbound.issuer,
        inbound.conversation.sender_wire_id(),
        &inbound.sender.claims(),
    )
    .await?;
    let authed = authenticated_user(&user);

    let context_id = inbound.conversation.context_id();

    let authz = AuthzRequest {
        entity: inbound.entity.clone(),
        user_id: user.id.clone(),
        actor: Some(Actor::user(user.id.clone())),
        client_id: None,
        access_scope: None,
        roles: user.roles.clone(),
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::generate(),
        session_id: None,
        context: AuthzContext::extension(
            format!("{}.message", inbound.conversation.platform()),
            json!({ "channel": inbound.conversation.channel_key() }),
        ),
        context_id: Some(context_id.clone()),
        task_id: None,
        act_chain: Vec::new(),
    };
    if let AuthzDecision::Deny { reason, policy } = ctx.authz_hook().evaluate(authz).await {
        return Ok(DispatchOutcome::Denied(format!("{policy}: {reason}")));
    }

    let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
    let token = mint_a2a_token(ctx, &authed, &session_id)?;

    let request = build_a2a_request(&inbound, &authed, &session_id, &token, &context_id)?;
    let reply = run_agent(ctx, &inbound.agent_name, request).await?;
    Ok(DispatchOutcome::Replied(reply))
}
