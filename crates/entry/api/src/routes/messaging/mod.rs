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

pub mod identity;

use std::sync::LazyLock;

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};
use systemprompt_agent::models::a2a::protocol::MessageSendConfiguration;
use systemprompt_agent::models::a2a::{
    A2aJsonRpcRequest, Message, MessageRole, MessageSendParams, Part, Task, TextPart,
};
use systemprompt_identifiers::{AgentName, ContextId, MessageId, SessionId, TraceId};
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::methods;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::{AuthzContext, AuthzDecision, AuthzRequest, EntityRef};
use systemprompt_users::User;

use crate::services::proxy::{ProxyEngine, ProxyKind, ProxyTarget};

use identity::resolve_or_link_user;

/// Cap on the A2A backend response body read into memory (1 MiB).
const MAX_A2A_RESPONSE_BYTES: usize = 1024 * 1024;

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
        act_chain: Vec::new(),
    };
    if let AuthzDecision::Deny { reason, policy } = ctx.authz_hook().evaluate(authz).await {
        return Ok(DispatchOutcome::Denied(format!("{policy}: {reason}")));
    }

    let context_id =
        ContextId::derived_from_messaging(inbound.platform, &inbound.org_id, &inbound.channel_id);
    let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
    let token = mint_a2a_token(ctx, &authed, &session_id)?;

    let request = build_a2a_request(&inbound, &authed, &session_id, &token, &context_id)?;
    let reply = run_agent(ctx, inbound.agent_name.as_str(), request).await?;
    Ok(DispatchOutcome::Replied(reply))
}

fn authenticated_user(user: &User) -> Result<AuthenticatedUser, MessagingError> {
    let id = uuid::Uuid::parse_str(user.id.as_str())
        .map_err(|e| MessagingError::Token(format!("user id is not a uuid: {e}")))?;
    Ok(AuthenticatedUser {
        id,
        username: user.name.clone(),
        email: user.email.clone(),
        permissions: vec![Permission::A2a],
        roles: user.roles.clone(),
        attributes: std::collections::BTreeMap::new(),
    })
}

/// Mint a per-user A2A bearer scoped to the `a2a` audience and permission, so
/// the proxy auth boundary accepts it for the agent backend.
fn mint_a2a_token(
    ctx: &AppContext,
    authed: &AuthenticatedUser,
    session_id: &SessionId,
) -> Result<String, MessagingError> {
    let config = JwtConfig {
        permissions: vec![Permission::A2a],
        audience: vec![JwtAudience::A2a],
        expires_in_hours: Some(1),
        resource: None,
        plugin_id: None,
    };
    let signing = JwtSigningParams {
        issuer: &ctx.config().jwt_issuer,
    };
    generate_jwt(
        authed,
        config,
        uuid::Uuid::new_v4().to_string(),
        session_id,
        &signing,
    )
    .map_err(|e| MessagingError::Token(e.to_string()))
}

/// Build the blocking `message/send` HTTP request, carrying the bearer header
/// and the `RequestContext` the proxy forwards.
fn build_a2a_request(
    inbound: &MessagingInbound,
    authed: &AuthenticatedUser,
    session_id: &SessionId,
    token: &str,
    context_id: &ContextId,
) -> Result<Request<Body>, MessagingError> {
    let message = Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: inbound.text.clone(),
        })],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };
    let params = MessageSendParams {
        message,
        configuration: Some(MessageSendConfiguration {
            accepted_output_modes: None,
            history_length: None,
            push_notification_config: None,
            blocking: Some(true),
        }),
        metadata: None,
    };
    let rpc = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        method: methods::SEND_MESSAGE.to_owned(),
        params: serde_json::to_value(&params)
            .map_err(|e| MessagingError::Dispatch(e.to_string()))?,
        id: RequestId::String(uuid::Uuid::new_v4().to_string()),
    };
    let body = serde_json::to_vec(&rpc).map_err(|e| MessagingError::Dispatch(e.to_string()))?;

    let mut request = Request::builder()
        .method("POST")
        .uri("/")
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .map_err(|e| MessagingError::Dispatch(e.to_string()))?;

    let req_context = RequestContext::new(
        session_id.clone(),
        TraceId::generate(),
        context_id.clone(),
        inbound.agent_name.clone(),
    )
    .with_user(authed.clone())
    .with_auth_token(token.to_owned());
    request.extensions_mut().insert(req_context);
    Ok(request)
}

async fn run_agent(
    ctx: &AppContext,
    agent: &str,
    request: Request<Body>,
) -> Result<String, MessagingError> {
    let target = ProxyTarget {
        service_name: agent,
        path: "",
        kind: ProxyKind::Agent,
    };
    let response = ProxyEngine::new()
        .proxy_request(target, request, ctx.clone())
        .await
        .map_err(|e| MessagingError::Dispatch(e.to_string()))?;

    let bytes = to_bytes(response.into_body(), MAX_A2A_RESPONSE_BYTES)
        .await
        .map_err(|e| MessagingError::Response(e.to_string()))?;
    let parsed: JsonRpcResponse<Task> =
        serde_json::from_slice(&bytes).map_err(|e| MessagingError::Response(e.to_string()))?;

    if let Some(err) = parsed.error {
        return Err(MessagingError::Dispatch(format!(
            "agent returned error {}: {}",
            err.code, err.message
        )));
    }
    Ok(reply_text(parsed.result.as_ref()))
}

/// The agent's terminal status message text, joined across text parts. Empty
/// when the agent produced no message.
fn reply_text(task: Option<&Task>) -> String {
    let Some(task) = task else {
        return String::new();
    };
    let Some(message) = task.status.message.as_ref() else {
        return String::new();
    };
    message
        .parts
        .iter()
        .filter_map(Part::as_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Test-only re-export of [`reply_text`], forwarded to its private definition
/// above so the agent-reply extraction can be unit-tested without driving the
/// full dispatch pipeline. Compiled only under `test-api`.
#[cfg(feature = "test-api")]
pub mod test_api {
    use systemprompt_agent::models::a2a::Task;

    #[must_use]
    pub fn reply_text(task: Option<&Task>) -> String {
        super::reply_text(task)
    }
}
