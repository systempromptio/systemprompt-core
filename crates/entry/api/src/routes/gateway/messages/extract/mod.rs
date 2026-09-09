//! Gateway request extraction and pre-dispatch authorization.
//!
//! Turns an inbound HTTP request into a validated `PreparedRequest`:
//! extracts the credential and required headers (see [`headers`]),
//! authenticates the principal, enforces session binding, parses the canonical
//! body, resolves the gateway route, and runs the pre-dispatch authz check (see
//! [`authz`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authz;
pub mod headers;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_identifiers::{
    ClientSessionId, ContextId, GatewayConversationId, SessionId, TraceId, UserId,
};

use super::RequestContext;
use super::auth::{AuthedPrincipal, authenticate};
use crate::services::gateway::protocol::canonical::CanonicalRequest;
use crate::services::gateway::protocol::inbound::InboundAdapter;
use authz::enforce_authz_pre_dispatch;
use headers::{
    classify_client_headers, optional_gateway_conversation_id, read_gateway_body,
    require_session_id,
};

pub use authz::{GatewayAuthzRequestInput, build_gateway_authz_request};
pub(super) use headers::ClientHeaders;
pub use headers::extract_credential;

#[derive(Debug, Default)]
pub struct RejectionPartial {
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub client_session_id: Option<ClientSessionId>,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub is_streaming: bool,
    pub body: Option<Bytes>,
}

pub(super) struct PreparedRequest {
    pub principal: AuthedPrincipal,
    pub body_bytes: Bytes,
    pub client_headers: ClientHeaders,
    pub gateway_request: CanonicalRequest,
    pub provider: String,
    pub upstream_model: String,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub gateway_conversation_id: GatewayConversationId,
    pub client_session_id: Option<ClientSessionId>,
}

pub(super) async fn extract_request_context(
    rc: &RequestContext<'_>,
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<PreparedRequest, (StatusCode, String)> {
    let gateway_config = rc
        .services
        .gateway_config()
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_owned()))?;

    let presented = extract_credential(request.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;

    let client_headers = classify_client_headers(request.headers());

    let session_id = require_session_id(request.headers())?;
    partial.session_id = Some(session_id.clone());
    let header_gateway_conversation = optional_gateway_conversation_id(request.headers())?;

    let principal = authenticate(
        &presented,
        &session_id,
        rc.jwt_extractor,
        rc.ctx,
        &rc.repos.execution_capabilities,
    )
    .await?;
    partial.user_id = Some(principal.user_id().clone());
    partial.trace_id = Some(principal.trace_id().clone());

    principal.enforce_session_binding(&session_id)?;

    let (body_bytes, mut gateway_request) = read_gateway_body(inbound, request, partial).await?;

    let (gateway_conversation_id, context_id, client_session_id) =
        derive_conversation(header_gateway_conversation, &gateway_request, partial)?;
    let route = gateway_config
        .resolve_route(&rc.services.providers, &gateway_request)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No gateway route matches model '{}'", gateway_request.model),
            )
        })?;
    partial.provider = Some(route.provider.as_str().to_owned());

    let wire = rc
        .services
        .providers
        .find_provider(route.provider.as_str())
        .map(|p| p.wire);
    rc.repos
        .thought_signatures
        .hydrate_request(&gateway_conversation_id, &mut gateway_request, wire)
        .await;

    let upstream_model = upstream_model_for(&rc.services.providers, &route, &gateway_request.model);

    enforce_authz_pre_dispatch(
        &principal,
        route.as_ref(),
        &gateway_request.model,
        rc.ctx.authz_hook(),
    )
    .await?;

    Ok(PreparedRequest {
        principal,
        body_bytes,
        client_headers,
        gateway_request,
        provider: route.provider.as_str().to_owned(),
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
        client_session_id,
    })
}

// Why: the gateway conversation id stays the per-thread prefix hash (it keys
// thought-signature hydration, and subagents inside one run have different
// prefixes), but the *context* a request lands in follows the caller's own
// session when it names one, so every thread of one Claude Code run shares
// the context its hook events already write to. An explicit header pins both.
pub fn derive_conversation(
    header_gateway_conversation: Option<GatewayConversationId>,
    gateway_request: &CanonicalRequest,
    partial: &mut RejectionPartial,
) -> Result<(GatewayConversationId, ContextId, Option<ClientSessionId>), (StatusCode, String)> {
    let header_supplied = header_gateway_conversation.is_some();
    let gateway_conversation_id = match header_gateway_conversation {
        Some(c) => c,
        None => gateway_request
            .derived_gateway_conversation_id()
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "request body has no messages; cannot derive gateway conversation id"
                        .to_owned(),
                )
            })?,
    };
    let client_session_id = gateway_request.client_session_id();
    let context_id = match (&client_session_id, header_supplied) {
        (Some(session), false) => ContextId::derived_from_client_session(session),
        _ => ContextId::derived_from_gateway_conversation(&gateway_conversation_id),
    };
    partial.context_id = Some(context_id.clone());
    partial.gateway_conversation_id = Some(gateway_conversation_id.clone());
    partial.client_session_id.clone_from(&client_session_id);
    Ok((gateway_conversation_id, context_id, client_session_id))
}

fn upstream_model_for(
    providers: &systemprompt_models::services::ProviderRegistry,
    route: &systemprompt_models::services::GatewayRoute,
    requested: &str,
) -> String {
    providers
        .find_provider(route.provider.as_str())
        .map_or_else(
            || route.effective_upstream_model(requested).to_owned(),
            |provider| {
                provider
                    .upstream_model_for(route.upstream_model.as_deref(), requested)
                    .to_owned()
            },
        )
}
