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
//! [`ContextId`] (derived from the conversation) ties multi-turn history
//! together instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod a2a;
pub mod identity;

use std::sync::LazyLock;

use serde_json::json;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId};
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::{AuthzContext, AuthzDecision, AuthzRequest, EntityRef};
use systemprompt_traits::SenderIdentity;

use a2a::{authenticated_user, build_a2a_request, mint_a2a_token, run_agent};
use identity::resolve_or_link_user;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

static GUARDED_CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    systemprompt_models::net::guarded_client(
        &systemprompt_models::net::GuardedClientConfig::default(),
    )
    .inspect_err(|e| tracing::error!(error = %e, "Guarded outbound http client unavailable"))
    .ok()
});

#[must_use]
pub fn http_client() -> reqwest::Client {
    CLIENT.clone()
}

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
    pub platform: &'static str,
    pub issuer: String,
    pub org_id: String,
    pub channel_id: String,
    pub external_user_id: String,
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
    #[error("identity resolution failed: {0}")]
    Identity(String),
    #[error("token minting failed: {0}")]
    Token(String),
    #[error("agent dispatch failed: {0}")]
    Dispatch(String),
    #[error("malformed agent response: {0}")]
    Response(String),
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
        &inbound.external_user_id,
        &inbound.sender.claims(),
    )
    .await?;
    let authed = authenticated_user(&user)?;

    let context_id =
        ContextId::derived_from_messaging(inbound.platform, &inbound.org_id, &inbound.channel_id);

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
            format!("{}.message", inbound.platform),
            json!({ "channel": inbound.channel_id }),
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
    let reply = run_agent(ctx, inbound.agent_name.as_str(), request).await?;
    Ok(DispatchOutcome::Replied(reply))
}
