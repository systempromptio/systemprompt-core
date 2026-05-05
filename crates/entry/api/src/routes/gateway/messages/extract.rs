use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use systemprompt_security::authz::{AuthzDecision, AuthzRequest, EntityKind};
use systemprompt_identifiers::{SessionId, TenantId, TraceId, UserId, headers as sp_headers};

use crate::services::gateway::models::AnthropicGatewayRequest;

use super::RequestContext;
use super::auth::{AuthedPrincipal, authenticate};

#[allow(clippy::struct_field_names)]
#[derive(Default)]
pub(super) struct RejectionPartial {
    pub user_id: Option<UserId>,
    pub tenant_id: Option<TenantId>,
    pub session_id: Option<SessionId>,
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
    pub gateway_request: AnthropicGatewayRequest,
    pub provider: String,
    pub upstream_model: String,
}

pub(super) async fn extract_request_context(
    rc: &RequestContext<'_>,
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

    let principal = authenticate(&presented, rc.jwt_extractor, rc.ctx, tenant_id).await?;
    partial.user_id = Some(principal.user_id.clone());
    partial.session_id.clone_from(&principal.session_id);
    partial.trace_id.clone_from(&principal.trace_id);

    let (body_bytes, gateway_request) = read_gateway_body(request, partial).await?;

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
    })
}

async fn read_gateway_body(
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<(Bytes, AnthropicGatewayRequest), (StatusCode, String)> {
    let body_bytes = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {e}"),
            )
        })?;
    partial.body = Some(body_bytes.clone());

    let gateway_request: AnthropicGatewayRequest =
        serde_json::from_slice(&body_bytes).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid request body: {e}"),
            )
        })?;
    partial.model = Some(gateway_request.model.clone());
    partial.max_tokens = Some(gateway_request.max_tokens);
    partial.is_streaming = gateway_request.stream.unwrap_or(false);
    Ok((body_bytes, gateway_request))
}

async fn enforce_authz_for_route(
    principal: &AuthedPrincipal,
    route: &systemprompt_models::profile::GatewayRoute,
    model: &str,
) -> Result<(), (StatusCode, String)> {
    let Some(hook) = systemprompt_security::authz::global_hook() else {
        tracing::error!(
            "authz hook not installed — denying gateway request (bootstrap order bug: install_from_governance_config must run before serving traffic)"
        );
        return Err((
            StatusCode::FORBIDDEN,
            "authz denied [authz_not_installed]: hook missing".to_string(),
        ));
    };
    let req = build_authz_request(principal, route, model)?;
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
) -> Result<AuthzRequest, (StatusCode, String)> {
    let tenant_id = principal.tenant_id.clone().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "tenant_id missing from claims".to_string(),
        )
    })?;
    let entity_id = if route.id.trim().is_empty() {
        systemprompt_models::profile::synthesize_route_id(
            &route.model_pattern,
            &route.provider,
            &route.endpoint,
        )
    } else {
        route.id.clone()
    };
    Ok(AuthzRequest {
        entity_type: EntityKind::GatewayRoute,
        entity_id,
        user_id: principal.user_id.clone(),
        tenant_id,
        roles: principal.roles.clone(),
        department: principal.department.clone(),
        trace_id: principal
            .trace_id
            .clone()
            .unwrap_or_else(TraceId::generate),
        context: serde_json::json!({"model": model}),
    })
}

pub(super) fn extract_credential(headers: &HeaderMap) -> Option<String> {
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
