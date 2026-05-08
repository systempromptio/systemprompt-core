use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_identifiers::{
    ContextId, GatewayConversationId, SessionId, TenantId, TraceId, UserId, headers as sp_headers,
};
use systemprompt_security::authz::{AuthzDecision, AuthzRequest, EntityKind};

use crate::services::gateway::protocol::canonical::CanonicalRequest;
use crate::services::gateway::protocol::inbound::InboundAdapter;

use super::RequestContext;
use super::auth::{AuthedPrincipal, authenticate};

#[allow(clippy::struct_field_names)]
#[derive(Default)]
pub(super) struct RejectionPartial {
    pub user_id: Option<UserId>,
    pub tenant_id: Option<TenantId>,
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
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_string()))?;

    let presented = extract_credential(request.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_string(),
        )
    })?;

    let tenant_id = request
        .headers()
        .get(sp_headers::TENANT_ID)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| TenantId::new(s.to_string()));
    partial.tenant_id.clone_from(&tenant_id);

    let session_id = require_session_id(request.headers())?;
    partial.session_id = Some(session_id.clone());
    let header_gateway_conversation = optional_gateway_conversation_id(request.headers())?;

    let principal = authenticate(&presented, rc.jwt_extractor, rc.ctx, tenant_id).await?;
    partial.user_id = Some(principal.user_id.clone());
    partial.trace_id.clone_from(&principal.trace_id);

    let (body_bytes, gateway_request) = read_gateway_body(inbound, request, partial).await?;

    let gateway_conversation_id = match header_gateway_conversation {
        Some(c) => c,
        None => gateway_request
            .derived_gateway_conversation_id()
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "request body has no messages; cannot derive gateway conversation id"
                        .to_string(),
                )
            })?,
    };
    let context_id = ContextId::derived_from_gateway_conversation(&gateway_conversation_id);
    partial.context_id = Some(context_id.clone());
    partial.gateway_conversation_id = Some(gateway_conversation_id.clone());

    let route = gateway_config
        .find_route(&gateway_request.model)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No gateway route matches model '{}'", gateway_request.model),
            )
        })?;
    partial.provider = Some(route.provider.clone());

    let upstream_model = route
        .effective_upstream_model(&gateway_request.model)
        .to_string();

    enforce_authz_for_route(&principal, route, &gateway_request.model).await?;

    Ok(PreparedRequest {
        principal,
        body_bytes,
        gateway_request,
        provider: route.provider.clone(),
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
    })
}

fn require_session_id(headers: &HeaderMap) -> Result<SessionId, (StatusCode, String)> {
    require_typed_header(headers, sp_headers::SESSION_ID, SessionId::new)
}

fn optional_gateway_conversation_id(
    headers: &HeaderMap,
) -> Result<Option<GatewayConversationId>, (StatusCode, String)> {
    let Some(raw) = headers.get(sp_headers::GATEWAY_CONVERSATION_ID) else {
        return Ok(None);
    };
    let raw = raw.to_str().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "invalid {} header: {e}",
                sp_headers::GATEWAY_CONVERSATION_ID
            ),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    GatewayConversationId::try_new(trimmed.to_string())
        .map(Some)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!(
                    "invalid {} header: {e}",
                    sp_headers::GATEWAY_CONVERSATION_ID
                ),
            )
        })
}

fn require_typed_header<T>(
    headers: &HeaderMap,
    name: &'static str,
    ctor: fn(String) -> T,
) -> Result<T, (StatusCode, String)> {
    let raw = headers
        .get(name)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("missing required {name} header"),
            )
        })?
        .to_str()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid {name} header: {e}"),
            )
        })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, format!("empty {name} header")));
    }
    Ok(ctor(trimmed.to_string()))
}

async fn read_gateway_body(
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<(Bytes, CanonicalRequest), (StatusCode, String)> {
    let body_bytes = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {e}"),
            )
        })?;
    partial.body = Some(body_bytes.clone());

    let canonical = inbound.parse_request(&body_bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid request body: {e}"),
        )
    })?;
    partial.model = Some(canonical.model.clone());
    partial.max_tokens = Some(canonical.max_tokens);
    partial.is_streaming = canonical.stream;
    Ok((body_bytes, canonical))
}

async fn enforce_authz_for_route(
    principal: &AuthedPrincipal,
    route: &systemprompt_models::profile::GatewayRoute,
    model: &str,
) -> Result<(), (StatusCode, String)> {
    let Some(hook) = systemprompt_security::authz::global_hook() else {
        tracing::error!(
            "authz hook not installed — denying gateway request (bootstrap order bug: \
             install_from_governance_config must run before serving traffic)"
        );
        return Err((
            StatusCode::FORBIDDEN,
            "authz denied [authz_not_installed]: hook missing".to_string(),
        ));
    };
    let req = build_authz_request(principal, route, model);
    match hook.evaluate(req).await {
        AuthzDecision::Allow => Ok(()),
        AuthzDecision::Deny { reason, policy } => Err((
            StatusCode::FORBIDDEN,
            format!("authz denied [{policy}]: {reason}"),
        )),
    }
}

fn build_authz_request(
    principal: &AuthedPrincipal,
    route: &systemprompt_models::profile::GatewayRoute,
    model: &str,
) -> AuthzRequest {
    let entity_id = if route.id.trim().is_empty() {
        systemprompt_models::profile::synthesize_route_id(
            &route.model_pattern,
            &route.provider,
            &route.endpoint,
        )
    } else {
        route.id.clone()
    };
    AuthzRequest {
        entity_type: EntityKind::GatewayRoute,
        entity_id,
        user_id: principal.user_id.clone(),
        roles: principal.roles.clone(),
        department: principal.department.clone(),
        trace_id: principal.trace_id.clone().unwrap_or_else(TraceId::generate),
        context: serde_json::json!({"model": model}),
    }
}

pub fn extract_credential(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("x-api-key")
        .or_else(|| headers.get("authorization"))
        .and_then(|v| v.to_str().ok())?;

    let trimmed = raw.trim_start_matches("Bearer ").trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
