//! The signed hop token the proxy attaches to every request it forwards to a
//! managed MCP server.
//!
//! The identity it carries is the one the edge verified for this server: the
//! authenticated user when the server requires OAuth, and no user (an
//! anonymous actor with the request's user id) when it does not. That user is
//! exactly the principal the edge binds the MCP session to, so the server's
//! session restore matches the binding. A follow-up request also carries the
//! MCP session id the edge confirmed is bound to that principal, so the
//! server refuses a token presented with any other session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_identifiers::{McpServerId, ServiceName};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{
    AuthenticatedUser, McpHopIdentity, TokenAuthority, TokenInteraction,
};
use systemprompt_models::execution::context::UserInteractionMode;

use super::super::backend::ProxyError;
use super::mcp_session::session_id_header;

#[derive(Debug)]
pub struct HopGrant<'a> {
    pub service_name: &'a ServiceName,
    pub context: &'a RequestContext,
    pub user: Option<&'a AuthenticatedUser>,
    pub request_headers: &'a HeaderMap,
}

pub fn hop_identity(grant: &HopGrant<'_>) -> McpHopIdentity {
    let context = grant.context;
    let user = grant.user.cloned();
    McpHopIdentity {
        user_id: user
            .as_ref()
            .map_or_else(|| context.user_id().clone(), |user| user.id.clone()),
        user,
        session_id: context.session_id().clone(),
        user_type: context.user_type(),
        client_id: context.client_id().cloned(),
        act_chain: context.act_chain().to_vec(),
        purpose: context.caller_purpose().map(str::to_owned),
        interaction: context
            .settings
            .user_interaction_mode
            .map(|mode| match mode {
                UserInteractionMode::Interactive => TokenInteraction::Interactive,
                UserInteractionMode::NonInteractive => TokenInteraction::NonInteractive,
            }),
        mcp_session: session_id_header(grant.request_headers).map(|id| id.as_str().to_owned()),
    }
}

pub(super) fn mint_hop_header(
    tokens: &dyn TokenAuthority,
    grant: &HopGrant<'_>,
) -> Result<HeaderValue, ProxyError> {
    let server = McpServerId::new(grant.service_name.as_str());
    let token = tokens
        .mint_mcp_hop(&server, &hop_identity(grant))
        .map_err(|source| ProxyError::HopTokenUnavailable {
            service: grant.service_name.to_string(),
            source,
        })?;
    HeaderValue::from_str(&token).map_err(|source| ProxyError::HopTokenHeader {
        service: grant.service_name.to_string(),
        source,
    })
}

pub(super) fn managed_headers(
    tokens: &dyn TokenAuthority,
    grant: &HopGrant<'_>,
) -> Result<HeaderMap, ProxyError> {
    let hop_token = mint_hop_header(tokens, grant)?;
    Ok(super::outbound::managed_mcp_headers(
        grant.request_headers,
        grant.context,
        &hop_token,
    ))
}
