#![allow(clippy::clone_on_ref_ptr)]
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use http::HeaderValue;
use systemprompt_ai::InsertSafetyFinding;
use systemprompt_ai::repository::AiSafetyFindingRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::profile::GatewayConfig;

use super::audit::{GatewayAudit, GatewayRequestContext};
use super::policy::{GatewayPolicySpec, PolicyResolver};
use super::protocol::canonical::CanonicalRequest;
use super::protocol::canonical_response::CanonicalResponse;
use super::protocol::inbound::InboundAdapter;
use super::protocol::outbound::{OutboundCtx, OutboundOutcome};
use super::registry::GatewayUpstreamRegistry;
use super::safety::{HeuristicScanner, SafetyScanner};
use super::{parse, quota, stream_tap};

pub const REQUEST_ID_HEADER: &str = "x-systemprompt-request-id";

#[derive(Debug, Clone, Copy)]
pub struct GatewayService;

/// Per-request inputs to [`GatewayService::dispatch`].
///
/// Bundles `request`, `raw_body`, `ctx`, and `inbound` so that the
/// surrounding env (`config`, `db`) stays as explicit arguments.
#[derive(Debug)]
pub struct DispatchInputs {
    pub request: CanonicalRequest,
    pub raw_body: Bytes,
    pub ctx: GatewayRequestContext,
    pub inbound: Arc<dyn InboundAdapter>,
}

impl GatewayService {
    pub async fn dispatch(
        config: &GatewayConfig,
        db: &DbPool,
        inputs: DispatchInputs,
    ) -> Result<Response<Body>> {
        let DispatchInputs {
            request,
            raw_body,
            ctx,
            inbound,
        } = inputs;
        if ctx.context_id.is_none() || ctx.session_id.is_none() {
            return Err(anyhow!(
                "gateway dispatch missing conversation binding (session_id + context_id)"
            ));
        }

        let route = config
            .find_route(&request.model)
            .ok_or_else(|| anyhow!("No gateway route matches model '{}'", request.model))?;

        let secrets = systemprompt_config::SecretsBootstrap::get()
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

        let is_streaming = request.stream;
        let ai_request_id = ctx.ai_request_id.clone();

        tracing::info!(
            ai_request_id = %ai_request_id,
            user_id = %ctx.user_id,
            tenant_id = ctx.tenant_id.as_ref().map_or("-", |t| t.as_str()),
            model = %request.model,
            provider = %route.provider,
            upstream = %route.endpoint,
            wire_protocol = %ctx.wire_protocol,
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
            GatewayAudit::new(db, ctx.clone()).map_err(|e| anyhow!("audit init failed: {e}"))?,
        );

        if let Err(e) = audit.open(&request, &raw_body).await {
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
                if let Err(e) = audit.fail(&msg).await {
                    tracing::warn!(error = %e, "quota audit fail failed");
                }
                return Err(QuotaExceeded {
                    message: msg,
                    retry_after_seconds: decision.window_seconds,
                }
                .into());
            }
        }

        run_request_safety_scan(db, &ai_request_id, &request).await;

        let upstream_model = route.effective_upstream_model(&request.model).to_string();
        let outbound_ctx = OutboundCtx {
            route,
            api_key: upstream_api_key,
            request: &request,
            upstream_model: &upstream_model,
        };

        let outcome = match upstream.send(outbound_ctx).await {
            Ok(o) => o,
            Err(e) => {
                if let Err(audit_err) = audit.fail(&e.to_string()).await {
                    tracing::warn!(error = %audit_err, "upstream audit fail failed");
                }
                return Err(e);
            },
        };

        let response = finalize(
            outcome,
            FinalizeCtx {
                audit: Arc::clone(&audit),
                db: db.clone(),
                ai_request_id: ai_request_id.clone(),
                policy,
                inbound,
                request_model: request.model.clone(),
            },
        )
        .await;
        Ok(attach_request_id(response, &ai_request_id))
    }
}

struct FinalizeCtx {
    audit: Arc<GatewayAudit>,
    db: DbPool,
    ai_request_id: AiRequestId,
    policy: GatewayPolicySpec,
    inbound: Arc<dyn InboundAdapter>,
    request_model: String,
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

async fn finalize(outcome: OutboundOutcome, fctx: FinalizeCtx) -> Response<Body> {
    let FinalizeCtx {
        audit,
        db,
        ai_request_id,
        policy,
        inbound,
        request_model,
    } = fctx;
    match outcome {
        OutboundOutcome::Buffered(canonical) => {
            let body_bytes = inbound.render_response(&canonical);
            let audit_clone = Arc::clone(&audit);
            let body_for_task = body_bytes.clone();
            tokio::spawn(async move {
                let canonical_for_task = canonical;
                let served_model = canonical_for_task.model.clone();
                if !served_model.is_empty() {
                    audit_clone.set_served_model(&served_model).await;
                }
                let (usage, tool_calls) = parse::extract_from_canonical(&canonical_for_task);
                if let Err(e) = audit_clone
                    .complete(usage, tool_calls, &canonical_for_task, &body_for_task)
                    .await
                {
                    tracing::warn!(error = %e, "buffered audit complete failed");
                }
                quota::post_update_tokens(
                    &db,
                    quota::PostUpdateParams {
                        tenant_id: audit_clone.ctx.tenant_id.as_ref(),
                        user_id: &audit_clone.ctx.user_id,
                        windows: &policy.quota_windows,
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                    },
                )
                .await;
                run_response_safety_scan(&db, &ai_request_id, &canonical_for_task).await;
            });
            Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body_bytes))
                .unwrap_or_else(|_| Response::new(Body::empty()))
        },
        OutboundOutcome::Streaming(stream) => {
            let body = stream_tap::tap(stream, Arc::clone(&inbound), request_model, audit);
            Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, inbound.streaming_content_type())
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
    request: &CanonicalRequest,
) {
    let scanner = HeuristicScanner;
    let findings = scanner.scan_request(request).await;
    if findings.is_empty() {
        return;
    }
    persist_findings(db, ai_request_id, findings).await;
}

async fn run_response_safety_scan(
    db: &DbPool,
    ai_request_id: &AiRequestId,
    response: &CanonicalResponse,
) {
    let scanner = HeuristicScanner;
    let findings = scanner.scan_response_final(response).await;
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
