//! Gateway request extraction and pre-dispatch authorization.
//!
//! Turns an inbound HTTP request into a validated [`PreparedRequest`]:
//! extracts the credential and required headers (see [`headers`]),
//! authenticates the principal, enforces session binding, parses the canonical
//! body, resolves the gateway route, and runs the pre-dispatch authz check (see
//! [`authz`]).

mod authz;
mod headers;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, GatewayConversationId, SessionId, TraceId, UserId};

use super::RequestContext;
use super::auth::{AuthedPrincipal, authenticate};
use crate::services::gateway::protocol::canonical::CanonicalRequest;
use crate::services::gateway::protocol::inbound::InboundAdapter;
use authz::enforce_authz_pre_dispatch;
use headers::{optional_gateway_conversation_id, read_gateway_body, require_session_id};

pub use authz::{GatewayAuthzRequestInput, build_gateway_authz_request};
pub use headers::extract_credential;

#[cfg(feature = "test-api")]
pub(super) mod test_api {
    pub use super::authz::enforce_authz_pre_dispatch;
    pub use super::headers::{
        optional_gateway_conversation_id, read_gateway_body, require_session_id,
    };
    pub use super::{RejectionPartial, derive_conversation};
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug, Default)]
pub struct RejectionPartial {
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
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
    pub gateway_request: CanonicalRequest,
    pub provider: String,
    pub upstream_model: String,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub gateway_conversation_id: GatewayConversationId,
}

pub(super) async fn extract_request_context(
    rc: &RequestContext<'_>,
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<PreparedRequest, (StatusCode, String)> {
    let gateway_config = rc
        .profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_owned()))?;

    let presented = extract_credential(request.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;

    let session_id = require_session_id(request.headers())?;
    partial.session_id = Some(session_id.clone());
    let header_gateway_conversation = optional_gateway_conversation_id(request.headers())?;

    let principal = authenticate(&presented, rc.jwt_extractor, rc.ctx).await?;
    partial.user_id = Some(principal.user_id().clone());
    partial.trace_id = Some(principal.trace_id().clone());

    principal.enforce_session_binding(&session_id)?;

    let (body_bytes, gateway_request) = read_gateway_body(inbound, request, partial).await?;

    let (gateway_conversation_id, context_id) =
        derive_conversation(header_gateway_conversation, &gateway_request, partial)?;

    let route = gateway_config
        .resolve_route(&rc.profile.providers, &gateway_request)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No gateway route matches model '{}'", gateway_request.model),
            )
        })?;
    partial.provider = Some(route.provider.as_str().to_owned());

    let upstream_model = route
        .effective_upstream_model(&gateway_request.model)
        .to_owned();

    enforce_authz_pre_dispatch(
        &principal,
        route.as_ref(),
        &gateway_request.model,
        &context_id,
        rc.ctx.authz_hook(),
    )
    .await?;

    Ok(PreparedRequest {
        principal,
        body_bytes,
        gateway_request,
        provider: route.provider.as_str().to_owned(),
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
    })
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn derive_conversation(
    header_gateway_conversation: Option<GatewayConversationId>,
    gateway_request: &CanonicalRequest,
    partial: &mut RejectionPartial,
) -> Result<(GatewayConversationId, ContextId), (StatusCode, String)> {
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
    let context_id = ContextId::derived_from_gateway_conversation(&gateway_conversation_id);
    partial.context_id = Some(context_id.clone());
    partial.gateway_conversation_id = Some(gateway_conversation_id.clone());
    Ok((gateway_conversation_id, context_id))
}
