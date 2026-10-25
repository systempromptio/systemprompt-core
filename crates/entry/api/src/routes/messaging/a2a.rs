//! The A2A backend leg of the messaging pipeline.
//!
//! Mints a per-user bearer, builds the blocking `SendMessage` request the
//! proxy forwards to the agent service, runs it through [`ProxyService`], and
//! extracts the agent's reply text.
//!
//! This is the only part of dispatch that speaks the A2A wire protocol; the
//! orchestration in [`super`] stays platform- and protocol-agnostic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};
use systemprompt_agent::models::a2a::{
    A2aJsonRpcRequest, Message, MessageRole, PROTOCOL_VERSION, Part, SendMessageConfiguration,
    SendMessageParams, SendMessageResult, VERSION_HEADER,
};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, JwtToken, MessageId, ServiceName, SessionId, TraceId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::methods;
use systemprompt_models::auth::{AuthenticatedUser, BaseRoles, JwtAudience, Permission};
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_runtime::AppContext;
use systemprompt_users::User;

use crate::services::proxy::{ProxyKind, ProxyService, ProxyTarget};

use super::{MessagingError, MessagingInbound};

const MAX_A2A_RESPONSE_BYTES: usize = 1024 * 1024;

pub fn permissions_for(roles: &[String]) -> Vec<Permission> {
    let held = if roles.iter().any(|role| role == BaseRoles::ADMIN) {
        Permission::Admin
    } else {
        Permission::User
    };
    vec![Permission::A2a, held]
}

pub(super) fn authenticated_user(user: &User) -> AuthenticatedUser {
    AuthenticatedUser {
        id: user.id.clone(),
        username: user.name.clone(),
        email: user.email.clone(),
        permissions: permissions_for(&user.roles),
        roles: user.roles.clone(),
        attributes: std::collections::BTreeMap::new(),
        account_kind: user.account_kind,
    }
}

pub(super) fn mint_a2a_token(
    ctx: &AppContext,
    authed: &AuthenticatedUser,
    session_id: &SessionId,
) -> Result<String, MessagingError> {
    let config = JwtConfig {
        permissions: authed.permissions.clone(),
        audience: vec![JwtAudience::A2a, JwtAudience::Mcp],
        expires_in: chrono::Duration::hours(1),
        resource: None,
        plugin_id: None,
        client_id: None,
        purpose: None,
    };
    let signing = JwtSigningParams {
        issuer: &ctx.config().jwt_issuer,
        keys: ctx.signing_keys(),
    };
    generate_jwt(
        authed,
        config,
        systemprompt_identifiers::AccessTokenId::generate(),
        session_id,
        &signing,
    )
    .map_err(MessagingError::Token)
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
        parts: vec![Part::text(inbound.text.clone())],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };
    let params = SendMessageParams {
        tenant: None,
        message,
        configuration: Some(SendMessageConfiguration {
            accepted_output_modes: None,
            history_length: None,
            return_immediately: false,
        }),
        metadata: None,
    };
    let rpc = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        method: methods::SEND_MESSAGE.to_owned(),
        params: serde_json::to_value(&params).map_err(MessagingError::Encode)?,
        id: RequestId::String(uuid::Uuid::new_v4().to_string()),
    };
    let body = serde_json::to_vec(&rpc).map_err(MessagingError::Encode)?;

    let mut request = Request::builder()
        .method("POST")
        .uri("/")
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(CONTENT_TYPE, "application/json")
        .header(VERSION_HEADER, PROTOCOL_VERSION)
        .body(Body::from(body))
        .map_err(MessagingError::Request)?;

    let req_context = RequestContext::new(
        session_id.clone(),
        TraceId::generate(),
        context_id.clone(),
        inbound.agent_name.clone(),
        Actor::user(authed.id.clone()),
    )
    .with_user(authed.clone())
    .with_auth_token(JwtToken::new(token));
    request.extensions_mut().insert(req_context);
    Ok(request)
}

pub(super) async fn run_agent(
    ctx: &AppContext,
    agent: &AgentName,
    request: Request<Body>,
) -> Result<String, MessagingError> {
    let service_name = ServiceName::new(agent.as_str());
    let target = ProxyTarget {
        service_name: &service_name,
        path: "",
        kind: ProxyKind::Agent,
    };
    let identities = crate::repository::proxy_identities(ctx.db_pool());
    let response = ProxyService::new(
        identities,
        Arc::clone(ctx.connector_tokens()),
        &ctx.services_state().view(),
        Arc::clone(ctx.config_arc()),
        Arc::clone(ctx.secrets()),
    )
    .proxy_request(target, request, ctx.clone())
    .await
    .map_err(MessagingError::Dispatch)?;

    let bytes = to_bytes(response.into_body(), MAX_A2A_RESPONSE_BYTES)
        .await
        .map_err(MessagingError::ResponseBody)?;
    let parsed: JsonRpcResponse<SendMessageResult> =
        serde_json::from_slice(&bytes).map_err(MessagingError::Response)?;

    if let Some(err) = parsed.error {
        return Err(MessagingError::AgentRejected {
            code: err.code,
            message: err.message,
        });
    }
    Ok(reply_text(parsed.result.as_ref()))
}

pub fn reply_text(result: Option<&SendMessageResult>) -> String {
    let message = match result {
        Some(SendMessageResult::Task(task)) => task.status.message.as_ref(),
        Some(SendMessageResult::Message(message)) => Some(&**message),
        None => None,
    };
    let Some(message) = message else {
        return String::new();
    };
    message
        .parts
        .iter()
        .filter_map(Part::as_text)
        .collect::<Vec<_>>()
        .join("\n")
}
