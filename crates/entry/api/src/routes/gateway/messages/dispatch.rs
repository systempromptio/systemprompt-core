use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;

use crate::services::gateway::audit::GatewayRequestContext;
use crate::services::gateway::protocol::inbound::InboundAdapter;
use crate::services::gateway::service::{DispatchInputs, GatewayService};

use super::RequestContext;
use super::extract::PreparedRequest;

pub(super) async fn dispatch_to_provider(
    rc: &RequestContext<'_>,
    inbound: Arc<dyn InboundAdapter>,
    prepared: PreparedRequest,
) -> Result<Response<Body>, (StatusCode, String)> {
    let PreparedRequest {
        principal,
        body_bytes,
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
        user_id: principal.user_id,
        session_id: Some(session_id),
        context_id,
        gateway_conversation_id: Some(gateway_conversation_id),
        trace_id: principal.trace_id,
        provider,
        model: upstream_model,
        max_tokens: Some(max_tokens),
        is_streaming,
        wire_protocol: inbound.wire_name().to_owned(),
    };

    let gateway_config = rc
        .profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_owned()))?;

    match GatewayService::dispatch(
        gateway_config,
        rc.ctx.db_pool(),
        DispatchInputs {
            request: gateway_request,
            raw_body: body_bytes,
            ctx: gateway_ctx,
            inbound,
        },
    )
    .await
    {
        Ok(resp) => Ok(resp),
        Err(e) => map_dispatch_error(&e),
    }
}

fn map_dispatch_error(e: &anyhow::Error) -> Result<Response<Body>, (StatusCode, String)> {
    if let Some(denied) = e.downcast_ref::<crate::services::gateway::service::PolicyDenied>() {
        return Err((StatusCode::FORBIDDEN, denied.to_string()));
    }
    if let Some(quota) = e.downcast_ref::<crate::services::gateway::service::QuotaExceeded>() {
        let mut resp = build_error_response(StatusCode::TOO_MANY_REQUESTS, &quota.message);
        if let Ok(v) = HeaderValue::from_str(&quota.retry_after_seconds.to_string()) {
            resp.headers_mut().insert("retry-after", v);
        }
        return Ok(resp);
    }
    Err((StatusCode::BAD_GATEWAY, e.to_string()))
}

pub(super) fn build_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        "{{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
    );
    match Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
    {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!(error = %e, status = %status, "Failed to build gateway error response");
            internal_error_response()
        },
    }
}

fn internal_error_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"error":"internal"}"#))
        .unwrap_or_else(|_| {
            let mut fallback = Response::new(Body::from(r#"{"error":"internal"}"#));
            *fallback.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            fallback
        })
}
