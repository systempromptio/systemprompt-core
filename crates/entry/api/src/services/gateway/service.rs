use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use http::HeaderValue;
use systemprompt_ai::repository::AiSafetyFindingRepository;
use systemprompt_ai::InsertSafetyFinding;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::profile::GatewayConfig;

use super::audit::{GatewayAudit, GatewayRequestContext};
use super::models::AnthropicGatewayRequest;
use super::parse;
use super::policy::{GatewayPolicySpec, PolicyResolver};
use super::quota;
use super::registry::GatewayUpstreamRegistry;
use super::safety::{HeuristicScanner, SafetyScanner};
use super::stream_tap;
use super::upstream::{UpstreamCtx, UpstreamOutcome, build_response};

pub const REQUEST_ID_HEADER: &str = "x-systemprompt-request-id";

#[derive(Debug, Clone, Copy)]
pub struct GatewayService;

impl GatewayService {
    pub async fn dispatch(
        config: &GatewayConfig,
        request: AnthropicGatewayRequest,
        raw_body: Bytes,
        ctx: GatewayRequestContext,
        db: &DbPool,
    ) -> Result<Response<Body>> {
        let route = config
            .find_route(&request.model)
            .ok_or_else(|| anyhow!("No gateway route matches model '{}'", request.model))?;

        let secrets = systemprompt_models::SecretsBootstrap::get()
            .map_err(|e| anyhow!("Secrets not available: {e}"))?;

        let upstream_api_key = secrets.get(&route.api_key_secret).ok_or_else(|| {
            anyhow!(
                "Gateway API key secret '{}' not configured",
                route.api_key_secret
            )
        })?;

        let upstream = GatewayUpstreamRegistry::global()
            .get(&route.provider)
            .ok_or_else(|| anyhow!("Gateway provider '{}' is not registered", route.provider))?;

        let is_streaming = request.stream.unwrap_or(false);
        let ai_request_id = ctx.ai_request_id.clone();

        tracing::info!(
            ai_request_id = %ai_request_id,
            user_id = %ctx.user_id,
            tenant_id = ctx.tenant_id.as_ref().map(|t| t.as_str()).unwrap_or("-"),
            model = %request.model,
            provider = %route.provider,
            upstream = %route.endpoint,
            streaming = is_streaming,
            "Gateway request dispatched"
        );

        let resolver = PolicyResolver::new(db)?;
        let policy = resolver.resolve(ctx.tenant_id.as_ref()).await;

        if !policy.model_allowed(&request.model) {
            tracing::warn!(
                ai_request_id = %ai_request_id,
                model = %request.model,
                "Gateway policy denied: model not in allowed list"
            );
            return Err(PolicyDenied(format!(
                "model '{}' is not permitted by gateway policy",
                request.model
            ))
            .into());
        }

        let audit = Arc::new(
            GatewayAudit::new(db, ctx.clone())
                .map_err(|e| anyhow!("audit init failed: {e}"))?,
        );

        if let Err(e) = audit.open(&raw_body).await {
            tracing::error!(error = %e, "audit open failed — proceeding without audit row");
        }

        if let Some(decision) = quota::precheck_and_reserve(
            db,
            ctx.tenant_id.as_ref(),
            &ctx.user_id,
            &policy.quota_windows,
        )
        .await?
        {
            if !decision.allow {
                let msg = format!(
                    "quota exceeded for window {}s (used {}/{:?})",
                    decision.window_seconds, decision.state.requests, decision.limit_requests
                );
                let _ = audit.fail(&msg).await;
                return Err(QuotaExceeded {
                    message: msg,
                    retry_after_seconds: decision.window_seconds,
                }
                .into());
            }
        }

        run_request_safety_scan(db, &ai_request_id, &request).await;

        let upstream_ctx = UpstreamCtx {
            route,
            api_key: upstream_api_key,
            raw_body,
            request: &request,
            is_streaming,
        };

        let outcome = match upstream.proxy(upstream_ctx).await {
            Ok(o) => o,
            Err(e) => {
                let _ = audit.fail(&e.to_string()).await;
                return Err(e);
            },
        };

        let response = finalize(
            outcome,
            Arc::clone(&audit),
            db.clone(),
            ai_request_id.clone(),
            policy,
        )
        .await;
        Ok(attach_request_id(response, &ai_request_id))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PolicyDenied(pub String);

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct QuotaExceeded {
    pub message: String,
    pub retry_after_seconds: i32,
}

async fn finalize(
    outcome: UpstreamOutcome,
    audit: Arc<GatewayAudit>,
    db: DbPool,
    ai_request_id: AiRequestId,
    policy: GatewayPolicySpec,
) -> Response<Body> {
    match outcome {
        UpstreamOutcome::Buffered {
            status,
            content_type,
            body,
        } => {
            let body_clone = body.clone();
            let audit_clone = Arc::clone(&audit);
            let db_clone = db.clone();
            let id_clone = ai_request_id.clone();
            let policy_clone = policy.clone();
            tokio::spawn(async move {
                if status.is_success() {
                    let (usage, tool_calls) = parse::extract_from_anthropic_response(&body_clone);
                    if let Err(e) = audit_clone.complete(usage, tool_calls, &body_clone).await {
                        tracing::warn!(error = %e, "buffered audit complete failed");
                    }
                    quota::post_update_tokens(
                        &db_clone,
                        audit_clone.ctx.tenant_id.as_ref(),
                        &audit_clone.ctx.user_id,
                        &policy_clone.quota_windows,
                        usage.input_tokens,
                        usage.output_tokens,
                    )
                    .await;
                    run_response_safety_scan(&db_clone, &id_clone, &body_clone).await;
                } else {
                    let err_msg = format!(
                        "upstream status {}: {}",
                        status.as_u16(),
                        String::from_utf8_lossy(&body_clone)
                    );
                    if let Err(e) = audit_clone.fail(&err_msg).await {
                        tracing::warn!(error = %e, "buffered audit fail update failed");
                    }
                }
            });
            build_response(UpstreamOutcome::Buffered {
                status,
                content_type,
                body,
            })
        },
        UpstreamOutcome::Streaming { status, stream } => {
            let body = stream_tap::tap(stream, Arc::clone(&audit));
            Response::builder()
                .status(status)
                .header(http::header::CONTENT_TYPE, "text/event-stream")
                .header("cache-control", "no-cache")
                .header("x-accel-buffering", "no")
                .body(body)
                .unwrap_or_else(|_| Response::new(Body::empty()))
        },
    }
}

async fn run_request_safety_scan(
    db: &DbPool,
    ai_request_id: &AiRequestId,
    request: &AnthropicGatewayRequest,
) {
    let scanner = HeuristicScanner;
    let findings = scanner.scan_request(request).await;
    if findings.is_empty() {
        return;
    }
    persist_findings(db, ai_request_id, findings).await;
}

async fn run_response_safety_scan(db: &DbPool, ai_request_id: &AiRequestId, body: &[u8]) {
    let scanner = HeuristicScanner;
    let findings = scanner.scan_response_final(body).await;
    if findings.is_empty() {
        return;
    }
    persist_findings(db, ai_request_id, findings).await;
}

async fn persist_findings(
    db: &DbPool,
    ai_request_id: &AiRequestId,
    findings: Vec<super::safety::Finding>,
) {
    let repo = match AiSafetyFindingRepository::new(db) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "safety findings repo init failed");
            return;
        },
    };
    for f in findings {
        let params = InsertSafetyFinding {
            ai_request_id,
            phase: f.phase,
            severity: f.severity.as_str(),
            category: &f.category,
            scanner: f.scanner,
            excerpt: f.excerpt.as_deref(),
        };
        if let Err(e) = repo.insert(params).await {
            tracing::warn!(error = %e, "safety finding insert failed");
        }
    }
}

fn attach_request_id(mut response: Response<Body>, id: &AiRequestId) -> Response<Body> {
    if let Ok(v) = HeaderValue::from_str(id.as_str()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, v);
    }
    response
}
