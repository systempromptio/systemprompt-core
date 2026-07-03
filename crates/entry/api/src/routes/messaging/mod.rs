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

mod a2a;
pub mod identity;

use std::sync::LazyLock;

use serde_json::json;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::{AuthzContext, AuthzDecision, AuthzRequest, EntityRef};

use a2a::{authenticated_user, build_a2a_request, mint_a2a_token, run_agent};
use identity::resolve_or_link_user;

/// Process-wide HTTP client for every outbound platform call. Cloning shares
/// the underlying connection pool; a fresh `reqwest::Client` per reply would
/// discard keep-alive connections and TLS sessions.
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

#[must_use]
pub fn http_client() -> reqwest::Client {
    CLIENT.clone()
}

/// Where a rendered reply is posted back to.
#[derive(Debug, Clone)]
pub enum ReplyTarget {
    /// Post into a channel/conversation by id (Slack `chat.postMessage`, Teams
    /// Bot Connector).
    Channel { id: String },
    /// Reply to a captured callback URL (Slack `response_url`).
    Url { url: String },
}

/// A surface-agnostic inbound message ready for dispatch. Per-platform routes
/// build this from their normalized payload; the dispatch core never sees a
/// Slack- or Teams-specific type.
#[derive(Debug, Clone)]
pub struct MessagingInbound {
    /// Stable platform tag (`"slack"` / `"teams"`), used in the derived
    /// `ContextId` and the authz context kind.
    pub platform: &'static str,
    /// The federated issuer that namespaces `external_user_id`.
    pub issuer: String,
    /// Workspace/tenant id — the org the conversation belongs to.
    pub org_id: String,
    /// Channel/conversation id within the org.
    pub channel_id: String,
    /// The platform's user id (already verified by the route).
    pub external_user_id: String,
    pub text: String,
    /// The agent resolved from app config for this conversation.
    pub agent_name: AgentName,
    /// The authz target — the workspace/tenant entity, config-seedable from
    /// `allowed_roles`.
    pub entity: EntityRef,
    /// Where the rendered reply is posted (owned by the route).
    pub reply: ReplyTarget,
}

/// The result of a dispatch, rendered back into the platform by the route.
#[derive(Debug, Clone)]
pub enum DispatchOutcome {
    /// Authorized; the agent's reply text (may be empty if the agent produced
    /// no message).
    Replied(String),
    /// Authorization denied; the reason is surfaced as an ephemeral refusal.
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

/// Run the shared inbound pipeline.
///
/// Links the platform sender to a governed identity, authorizes against the
/// workspace/tenant entity, mints a per-user A2A token, dispatches a blocking
/// `message/send` through the proxy, and returns the agent's reply.
pub async fn dispatch_messaging(
    ctx: &AppContext,
    inbound: MessagingInbound,
) -> Result<DispatchOutcome, MessagingError> {
    let user = resolve_or_link_user(ctx, &inbound.issuer, &inbound.external_user_id).await?;
    let authed = authenticated_user(&user)?;

    let context_id =
        ContextId::derived_from_messaging(inbound.platform, &inbound.org_id, &inbound.channel_id);

    let authz = AuthzRequest {
        entity: inbound.entity.clone(),
        user_id: user.id.clone(),
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

/// Test-only re-export of the agent-reply extraction, so it can be unit-tested
/// without driving the full dispatch pipeline.
#[cfg(feature = "test-api")]
pub mod test_api {
    use systemprompt_agent::models::a2a::Task;

    #[must_use]
    pub fn reply_text(task: Option<&Task>) -> String {
        super::a2a::reply_text(task)
    }
}
