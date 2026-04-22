use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use std::sync::Arc;
use systemprompt_identifiers::{
    AiRequestId, JwtToken, SessionId, TenantId, TraceId, UserId, headers as sp_headers,
};
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::AppContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::gateway::GatewayService;
use crate::services::gateway::audit::GatewayRequestContext;
use crate::services::gateway::models::AnthropicGatewayRequest;
use crate::services::middleware::JwtContextExtractor;

struct AuthedPrincipal {
    user_id: UserId,
    tenant_id: Option<TenantId>,
    session_id: Option<SessionId>,
    trace_id: Option<TraceId>,
}

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request<Body>,
) -> Response<Body> {
    match handle_inner(jwt_extractor, ctx, request).await {
        Ok(resp) => resp,
        Err((status, message)) => {
            tracing::warn!(status = %status, message = %message, "Gateway request rejected");
            build_error_response(status, &message)
        },
    }
}

async fn handle_inner(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request<Body>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    let gateway_config = profile
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

    let principal = authenticate(&presented, &jwt_extractor, &ctx, tenant_id).await?;

    let body_bytes = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {e}"),
            )
        })?;

    let gateway_request: AnthropicGatewayRequest =
        serde_json::from_slice(&body_bytes).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid request body: {e}"),
            )
        })?;

    let route = gateway_config
        .find_route(&gateway_request.model)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!(
                    "No gateway route matches model '{}'",
                    gateway_request.model
                ),
            )
        })?;

    let gateway_ctx = GatewayRequestContext {
        ai_request_id: AiRequestId::generate(),
        user_id: principal.user_id,
        tenant_id: principal.tenant_id,
        session_id: principal.session_id,
        trace_id: principal.trace_id,
        provider: route.provider.clone(),
        model: gateway_request.model.clone(),
        max_tokens: Some(gateway_request.max_tokens),
        is_streaming: gateway_request.stream.unwrap_or(false),
    };

    match GatewayService::dispatch(
        gateway_config,
        gateway_request,
        body_bytes,
        gateway_ctx,
        ctx.db_pool(),
    )
    .await
    {
        Ok(resp) => Ok(resp),
        Err(e) => {
            if let Some(denied) = e.downcast_ref::<crate::services::gateway::service::PolicyDenied>()
            {
                return Err((StatusCode::FORBIDDEN, denied.to_string()));
            }
            if let Some(quota) =
                e.downcast_ref::<crate::services::gateway::service::QuotaExceeded>()
            {
                let mut resp = build_error_response(StatusCode::TOO_MANY_REQUESTS, &quota.message);
                if let Ok(v) = HeaderValue::from_str(&quota.retry_after_seconds.to_string()) {
                    resp.headers_mut().insert("retry-after", v);
                }
                return Ok(resp);
            }
            Err((StatusCode::BAD_GATEWAY, e.to_string()))
        },
    }
}

async fn authenticate(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
    tenant_id: Option<TenantId>,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential.starts_with(API_KEY_PREFIX) {
        let service = ApiKeyService::new(ctx.db_pool()).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("API key service unavailable: {e}"),
            )
        })?;
        let record = service.verify(credential).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("API key verification failed: {e}"),
            )
        })?;
        return match record {
            Some(rec) => Ok(AuthedPrincipal {
                user_id: rec.user_id,
                tenant_id,
                session_id: None,
                trace_id: Some(TraceId::generate()),
            }),
            None => Err((
                StatusCode::UNAUTHORIZED,
                "Invalid or revoked API key".to_string(),
            )),
        };
    }

    let jwt_token = JwtToken::new(credential);
    let rc = jwt_extractor
        .extract_for_gateway(&jwt_token)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(AuthedPrincipal {
        user_id: rc.auth.user_id.clone(),
        tenant_id,
        session_id: if rc.request.session_id.as_str().is_empty() {
            None
        } else {
            Some(rc.request.session_id.clone())
        },
        trace_id: Some(rc.execution.trace_id.clone()),
    })
}

fn build_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        "{{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
    );
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn extract_credential(headers: &HeaderMap) -> Option<String> {
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
