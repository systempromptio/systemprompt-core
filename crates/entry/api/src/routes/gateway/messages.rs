use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_ai::models::ai_request_record::AiRequestRecord;
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, UpsertPayloadParams,
};
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

#[allow(clippy::struct_field_names)]
struct AuthedPrincipal {
    user_id: UserId,
    tenant_id: Option<TenantId>,
    session_id: Option<SessionId>,
    trace_id: Option<TraceId>,
}

#[derive(Default)]
struct RejectionPartial {
    user_id: Option<UserId>,
    tenant_id: Option<TenantId>,
    session_id: Option<SessionId>,
    trace_id: Option<TraceId>,
    provider: Option<String>,
    model: Option<String>,
    max_tokens: Option<u32>,
    is_streaming: bool,
    body: Option<Bytes>,
}

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request<Body>,
) -> Response<Body> {
    let ai_request_id = AiRequestId::generate();
    let mut partial = RejectionPartial::default();
    match handle_inner(
        jwt_extractor,
        ctx.clone(),
        request,
        &ai_request_id,
        &mut partial,
    )
    .await
    {
        Ok(resp) => resp,
        Err((status, message)) => {
            tracing::warn!(status = %status, message = %message, ai_request_id = %ai_request_id, "Gateway request rejected");
            persist_rejection(&ctx, &ai_request_id, &partial, status, &message).await;
            build_error_response(status, &message)
        },
    }
}

async fn persist_rejection(
    ctx: &AppContext,
    ai_request_id: &AiRequestId,
    partial: &RejectionPartial,
    status: StatusCode,
    message: &str,
) {
    let repo = match AiRequestRepository::new(ctx.db_pool()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "rejection audit: repo unavailable");
            return;
        },
    };
    let user_id = partial
        .user_id
        .clone()
        .unwrap_or_else(|| UserId::new("anonymous"));
    let provider = partial
        .provider
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let model = partial
        .model
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    let mut builder = AiRequestRecord::builder(ai_request_id.clone(), user_id)
        .provider(provider)
        .model(model)
        .streaming(partial.is_streaming);
    if let Some(t) = &partial.tenant_id {
        builder = builder.tenant_id(t.clone());
    }
    if let Some(s) = &partial.session_id {
        builder = builder.session_id(s.clone());
    }
    if let Some(t) = &partial.trace_id {
        builder = builder.trace_id(t.clone());
    }
    if let Some(mt) = partial.max_tokens {
        builder = builder.max_tokens(mt);
    }
    let record = match builder.build() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "rejection audit: build failed");
            AiRequestRecord::minimal_fallback(ai_request_id.as_str().to_string())
        },
    };
    if let Err(e) = repo.insert_with_id(ai_request_id, &record).await {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: insert failed");
        return;
    }
    let err_msg = format!("HTTP {}: {message}", status.as_u16());
    if let Err(e) = repo.update_error(ai_request_id, &err_msg).await {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: update_error failed");
    }

    if let Some(body) = partial.body.as_ref() {
        match AiRequestPayloadRepository::new(ctx.db_pool()) {
            Ok(payloads) => {
                let bytes_len = body.len().min(i32::MAX as usize) as i32;
                let body_json = serde_json::from_slice::<serde_json::Value>(body).ok();
                let excerpt = if body_json.is_none() {
                    Some(String::from_utf8_lossy(body).to_string())
                } else {
                    None
                };
                if let Err(e) = payloads
                    .upsert_request(
                        ai_request_id,
                        UpsertPayloadParams {
                            body: body_json.as_ref(),
                            excerpt: excerpt.as_deref(),
                            truncated: false,
                            bytes: Some(bytes_len),
                        },
                    )
                    .await
                {
                    tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: payload insert failed");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "rejection audit: payload repo unavailable");
            },
        }
    }
}

async fn handle_inner(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request<Body>,
    ai_request_id: &AiRequestId,
    partial: &mut RejectionPartial,
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
    partial.tenant_id = tenant_id.clone();

    let principal = authenticate(&presented, &jwt_extractor, &ctx, tenant_id).await?;
    partial.user_id = Some(principal.user_id.clone());
    partial.session_id = principal.session_id.clone();
    partial.trace_id = principal.trace_id.clone();

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

    let route = gateway_config
        .find_route(&gateway_request.model)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No gateway route matches model '{}'", gateway_request.model),
            )
        })?;
    partial.provider = Some(route.provider.clone());

    let gateway_ctx = GatewayRequestContext {
        ai_request_id: ai_request_id.clone(),
        user_id: principal.user_id,
        tenant_id: principal.tenant_id,
        session_id: principal.session_id,
        trace_id: principal.trace_id,
        provider: route.provider.clone(),
        model: route
            .effective_upstream_model(&gateway_request.model)
            .to_string(),
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
            if let Some(denied) =
                e.downcast_ref::<crate::services::gateway::service::PolicyDenied>()
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
        trace_id: Some(rc.execution.trace_id),
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
