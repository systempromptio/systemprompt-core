//! The A2A backend leg of the messaging pipeline: mint a per-user bearer, build
//! the blocking `message/send` request the proxy forwards to the agent service,
//! run it through [`ProxyEngine`], and extract the agent's reply text.
//!
//! This is the only part of dispatch that speaks the A2A wire protocol; the
//! orchestration in [`super`] stays platform- and protocol-agnostic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};
use systemprompt_agent::models::a2a::protocol::MessageSendConfiguration;
use systemprompt_agent::models::a2a::{
    A2aJsonRpcRequest, Message, MessageRole, MessageSendParams, Part, Task, TextPart,
};
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TraceId};
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::methods;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_runtime::AppContext;
use systemprompt_users::User;

use crate::services::proxy::{ProxyEngine, ProxyKind, ProxyTarget};

use super::{MessagingError, MessagingInbound};

const MAX_A2A_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) fn authenticated_user(user: &User) -> Result<AuthenticatedUser, MessagingError> {
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
pub(super) fn mint_a2a_token(
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

pub(super) fn build_a2a_request(
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

pub(super) async fn run_agent(
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

/// Empty when the agent produced no terminal status message.
pub(super) fn reply_text(task: Option<&Task>) -> String {
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
