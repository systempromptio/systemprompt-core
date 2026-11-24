//! The A2A backend leg of the messaging pipeline.
//!
//! Mints a per-user bearer ([`A2aTokenIssuer`]), builds the blocking
//! `SendMessage` request the proxy forwards to the agent service, sends it
//! through the kernel's sealed agent dispatch (the API's agent proxy), and
//! extracts the agent's reply text.
//!
//! This is the only part of dispatch that speaks the A2A wire protocol; the
//! orchestration in [`super`] stays platform- and protocol-agnostic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};
use systemprompt_agent::models::a2a::{
    A2aJsonRpcRequest, Message, MessageRole, Part, SendMessageConfiguration, SendMessageParams,
    SendMessageResult,
};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, JwtToken, MessageId, SessionId, TraceId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::dispatch::AgentDispatch;
use systemprompt_models::a2a::methods;
use systemprompt_models::auth::{AuthenticatedUser, BaseRoles, JwtAudience, Permission};
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::MessagingInbound;
use systemprompt_users::User;

use super::MessagingError;

pub fn permissions_for(roles: &[String]) -> Vec<Permission> {
    vec![Permission::A2a, BaseRoles::permission_for(roles)]
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

/// Mints the one-hour A2A and MCP bearer a dispatched message runs under, for
/// the user the dispatcher resolved, and nothing else.
pub(super) struct A2aTokenIssuer {
    pub(super) issuer: String,
    pub(super) keys: SigningKeys,
}

impl A2aTokenIssuer {
    pub(super) fn mint(
        &self,
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
            interaction: None,
            browser_session: false,
            ui_session: false,
            auth_time: None,
        };
        let signing = JwtSigningParams {
            issuer: &self.issuer,
            keys: &self.keys,
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
}

pub(super) fn build_a2a_request(
    inbound: &MessagingInbound,
    authed: &AuthenticatedUser,
    session_id: &SessionId,
    token: &str,
    context_id: &ContextId,
) -> Result<(Vec<u8>, RequestContext), MessagingError> {
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

    let req_context = RequestContext::new(
        session_id.clone(),
        TraceId::generate(),
        context_id.clone(),
        inbound.agent_name.clone(),
        Actor::user(authed.id.clone()),
    )
    .with_user(authed.clone())
    .with_auth_token(JwtToken::new(token));
    Ok((body, req_context))
}

pub(super) async fn run_agent(
    dispatch: &dyn AgentDispatch,
    agent: &AgentName,
    request: (Vec<u8>, RequestContext),
) -> Result<String, MessagingError> {
    let (body, context) = request;
    let bytes = dispatch
        .send(agent, body, context)
        .await
        .map_err(MessagingError::Dispatch)?;
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
