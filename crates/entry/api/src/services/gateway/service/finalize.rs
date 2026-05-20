//! Response finalization: turns an `OutboundOutcome` into an HTTP response,
//! spawns the audit-completion task, runs safety scans, and stamps the
//! request-id header.

use std::sync::Arc;

use axum::body::Body;
use axum::response::Response;
use http::HeaderValue;
use systemprompt_ai::InsertSafetyFinding;
use systemprompt_ai::repository::AiSafetyFindingRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;

use super::super::audit::GatewayAudit;
use super::super::policy::GatewayPolicySpec;
use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::canonical_response::CanonicalResponse;
use super::super::protocol::inbound::InboundAdapter;
use super::super::protocol::outbound::OutboundOutcome;
use super::super::safety::{Finding, HeuristicScanner, SafetyScanner};
use super::super::{parse, quota, stream_tap};
use super::REQUEST_ID_HEADER;

pub(super) struct FinalizeCtx {
    pub(super) audit: Arc<GatewayAudit>,
    pub(super) db: DbPool,
    pub(super) ai_request_id: AiRequestId,
    pub(super) policy: GatewayPolicySpec,
    pub(super) inbound: Arc<dyn InboundAdapter>,
    pub(super) request_model: String,
}

pub(super) async fn finalize(outcome: OutboundOutcome, fctx: FinalizeCtx) -> Response<Body> {
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

pub(super) async fn run_request_safety_scan(
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

async fn persist_findings(db: &DbPool, ai_request_id: &AiRequestId, findings: Vec<Finding>) {
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

pub(super) fn attach_request_id(mut response: Response<Body>, id: &AiRequestId) -> Response<Body> {
    if let Ok(v) = HeaderValue::from_str(id.as_str()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, v);
    }
    response
}
