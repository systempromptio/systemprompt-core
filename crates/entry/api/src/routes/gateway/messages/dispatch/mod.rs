//! Gateway message dispatch: route resolution and upstream invocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;

use crate::services::gateway::audit::GatewayRequestContext;
use crate::services::gateway::protocol::inbound::InboundAdapter;
use crate::services::gateway::service::{DispatchInputs, GatewayService};

use super::RequestContext;
use super::extract::PreparedRequest;

mod errors;

pub use self::errors::map_upstream_error;

#[cfg(feature = "test-api")]
pub use self::errors::{
    build_error_response, build_policy_denial, classify_dispatch_error, error_type_for,
    map_dispatch_error, policy_denial_message,
};

#[cfg(not(feature = "test-api"))]
pub(super) use self::errors::{build_error_response, error_type_for, map_dispatch_error};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub struct RejectionError {
    pub status: StatusCode,
    pub message: String,
    pub persist: bool,
}

pub(super) async fn dispatch_to_provider(
    rc: &RequestContext<'_>,
    inbound: Arc<dyn InboundAdapter>,
    prepared: PreparedRequest,
) -> Result<Response<Body>, RejectionError> {
    let PreparedRequest {
        principal,
        body_bytes,
        client_headers,
        gateway_request,
        provider,
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
    } = prepared;

    let max_tokens = gateway_request.max_tokens;
    let is_streaming = gateway_request.stream;

    let gateway_ctx = GatewayRequestContext {
        ai_request_id: rc.ai_request_id.clone(),
        user_id: principal.user_id().clone(),
        session_id: Some(session_id),
        context_id,
        gateway_conversation_id: Some(gateway_conversation_id),
        trace_id: Some(principal.trace_id().clone()),
        access_scope: principal.access_scope(),
        client_id: principal.client_id().cloned(),
        provider,
        requested_model: Some(gateway_request.model.clone()),
        model: upstream_model,
        max_tokens: Some(max_tokens),
        is_streaming,
        wire_protocol: inbound.wire_name().to_owned(),
        access_log: rc.access_log.clone(),
    };

    let gateway_config = rc
        .profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .ok_or_else(|| RejectionError {
            status: StatusCode::NOT_FOUND,
            message: "Gateway not enabled".to_owned(),
            persist: true,
        })?;

    match GatewayService::dispatch(
        gateway_config,
        &rc.profile.providers,
        rc.ctx.db_pool(),
        rc.repos,
        DispatchInputs {
            request: gateway_request,
            raw_body: body_bytes,
            ctx: gateway_ctx,
            inbound,
            forward_headers: client_headers.forward,
            identity_headers: client_headers.identity,
        },
    )
    .await
    {
        Ok(resp) => Ok(resp),
        Err(e) => map_dispatch_error(e),
    }
}
