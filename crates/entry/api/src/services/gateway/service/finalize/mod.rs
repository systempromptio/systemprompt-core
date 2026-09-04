//! Response finalization: turns an `OutboundOutcome` into an HTTP response,
//! spawns the audit-completion task, runs safety scans, and stamps the
//! request-id header.
//!
//! Response-phase scanning reads the bytes the *client* receives, which is the
//! surface that matters for egress and the only one both lanes agree on. A
//! buffered reply can therefore be denied before it is served, gated on
//! `SafetyConfig::block_response_categories`; audit, quota, and cost stay in
//! the spawned completion task so only the scan sits on the critical path.
//!
//! Streaming stays audit-only. The frames are already flowing by the time a
//! whole-response scan can run, so terminating mid-stream would still leak
//! everything sent up to that point — the block would be theatre, not
//! containment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::Body;
use axum::response::Response;
use http::HeaderValue;
use systemprompt_ai::{OverrideAction, OverrideContext, OverrideEngine};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, ModelId, ProviderId};
use systemprompt_models::services::GatewayConfig;
use systemprompt_models::wire::inspect::{SurfaceBudget, string_leaves};

pub(super) mod safety;

#[cfg(feature = "test-api")]
pub use self::safety::dedupe_findings;
pub(in crate::services::gateway) use self::safety::{
    request_finding_blocks, run_request_safety_scan, run_response_safety_scan,
};

use super::super::audit::GatewayAudit;
use super::super::policy::GatewayPolicySpec;
use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::canonical_response::CanonicalResponse;
use super::super::protocol::inbound::InboundAdapter;
use super::super::protocol::outbound::OutboundOutcome;
use super::super::signature_cache::ThoughtSignatureCache;
use super::super::{parse, quota, stream_tap};
use super::REQUEST_ID_HEADER;

pub(super) struct FinalizeCtx {
    pub(super) audit: Arc<GatewayAudit>,
    pub(super) db: DbPool,
    pub(super) repos: crate::services::gateway::GatewayRepositories,
    pub(super) ai_request_id: AiRequestId,
    pub(super) policy: GatewayPolicySpec,
    pub(super) inbound: Arc<dyn InboundAdapter>,
    pub(super) request_model: String,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub async fn apply_system_prompt_override(
    config: &GatewayConfig,
    provider: &ProviderId,
    upstream_model: &str,
    request: &mut CanonicalRequest,
) -> Option<String> {
    let engine = OverrideEngine::global();
    if config.system_prompt_overrides.is_empty() && !engine.has_extensions() {
        return None;
    }
    let ctx = OverrideContext::builder(provider.clone(), ModelId::new(&request.model))
        .upstream_model(ModelId::new(upstream_model))
        .current_system(request.system.clone())
        .build();
    let resolution = engine.resolve(&config.system_prompt_overrides, &ctx).await;
    let descriptor = resolution.audit_descriptor();
    match resolution.action {
        OverrideAction::Replace(prompt) => request.system = Some(prompt),
        OverrideAction::Strip => request.system = None,
        OverrideAction::Passthrough => {},
    }
    descriptor
}

pub(super) async fn finalize(outcome: OutboundOutcome, fctx: FinalizeCtx) -> Response<Body> {
    let FinalizeCtx {
        audit,
        db,
        repos,
        ai_request_id,
        policy,
        inbound,
        request_model,
    } = fctx;
    let tap_ctx = stream_tap::TapFinalizeCtx {
        db,
        repos,
        policy,
        ai_request_id,
    };
    match outcome {
        OutboundOutcome::Buffered(canonical) => {
            let canonical = *canonical;
            let body = inbound.render_response(&canonical);
            finalize_buffered(canonical, body, "application/json", &audit, tap_ctx).await
        },
        OutboundOutcome::RawBuffered {
            body,
            content_type,
            canonical,
        } => {
            let content_type = content_type.unwrap_or_else(|| "application/json".to_owned());
            finalize_buffered(*canonical, body, &content_type, &audit, tap_ctx).await
        },
        OutboundOutcome::RawStreaming {
            content_type,
            stream,
        } => streaming_response(
            stream_tap::tap_raw(stream, audit, tap_ctx),
            content_type
                .as_deref()
                .unwrap_or_else(|| inbound.streaming_content_type()),
        ),
        OutboundOutcome::Streaming(stream) => {
            let content_type = inbound.streaming_content_type();
            let body = stream_tap::tap(stream, Arc::clone(&inbound), request_model, audit, tap_ctx);
            streaming_response(body, content_type)
        },
    }
}

async fn finalize_buffered(
    mut canonical: CanonicalResponse,
    body: bytes::Bytes,
    content_type: &str,
    audit: &Arc<GatewayAudit>,
    tap_ctx: stream_tap::TapFinalizeCtx,
) -> Response<Body> {
    canonical.received_surface = string_leaves(&body, SurfaceBudget::default());
    if tap_ctx.policy.safety.block_response_categories.is_empty() {
        spawn_buffered_completion(canonical, body.clone(), audit, tap_ctx, false);
        return buffered_response(body, content_type);
    }
    let safety = tap_ctx.policy.safety.clone();
    let findings = run_response_safety_scan(
        &tap_ctx.repos.safety_findings,
        &tap_ctx.ai_request_id,
        &canonical,
        &safety,
    )
    .await;
    // Why: warn mode still scans and still persists, so the report shows what
    // the response block list would have caught. It only skips the refusal.
    let blocked = findings
        .iter()
        .find(|f| !safety.mode.is_warn() && safety.block_response_categories.contains(&f.category))
        .map(|f| (f.category.clone(), f.scanner));
    spawn_buffered_completion(canonical, body.clone(), audit, tap_ctx, true);
    match blocked {
        Some((category, scanner)) => {
            tracing::warn!(
                category = %category,
                scanner = %scanner,
                "Gateway blocked response by safety policy"
            );
            safety_block_response(&category)
        },
        None => buffered_response(body, content_type),
    }
}

fn safety_block_response(category: &str) -> Response<Body> {
    let message = format!("response blocked by safety policy: category '{category}'");
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        "{{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
    );
    Response::builder()
        .status(http::StatusCode::FORBIDDEN)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn spawn_buffered_completion(
    canonical: CanonicalResponse,
    body: bytes::Bytes,
    audit: &Arc<GatewayAudit>,
    tap_ctx: stream_tap::TapFinalizeCtx,
    response_scanned: bool,
) {
    tokio::spawn(buffered_completion(
        canonical,
        body,
        Arc::clone(audit),
        tap_ctx,
        response_scanned,
    ));
}

fn buffered_response(body: bytes::Bytes, content_type: &str) -> Response<Body> {
    Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn streaming_response(body: Body, content_type: &str) -> Response<Body> {
    Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, content_type)
        .header("cache-control", "no-cache")
        .header("x-accel-buffering", "no")
        .body(body)
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

async fn buffered_completion(
    canonical: CanonicalResponse,
    body: bytes::Bytes,
    audit: Arc<GatewayAudit>,
    ctx: stream_tap::TapFinalizeCtx,
    response_scanned: bool,
) {
    match &audit.ctx.gateway_conversation_id {
        Some(conversation) => {
            ctx.repos
                .thought_signatures
                .store_from_response(conversation, &canonical)
                .await;
        },
        None => ThoughtSignatureCache::note_uncacheable_response(&canonical, "no_conversation_id"),
    }
    let served_model = canonical.model.clone();
    if !served_model.is_empty() {
        audit.set_served_model(&served_model).await;
    }
    let (usage, tool_calls) = parse::extract_from_canonical(&canonical);
    let cost_microdollars = match audit.complete(usage, tool_calls, &canonical, &body).await {
        Ok(cost) => cost,
        Err(e) => {
            tracing::warn!(error = %e, "buffered audit complete failed");
            0
        },
    };
    quota::post_update_tokens(
        &ctx.db,
        &ctx.repos.quota_buckets,
        quota::PostUpdateParams {
            user_id: &audit.ctx.user_id,
            windows: &ctx.policy.quota_windows,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost_microdollars,
        },
    )
    .await;
    if !response_scanned {
        run_response_safety_scan(
            &ctx.repos.safety_findings,
            &ctx.ai_request_id,
            &canonical,
            &ctx.policy.safety,
        )
        .await;
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn attach_request_id(mut response: Response<Body>, id: &AiRequestId) -> Response<Body> {
    if let Ok(v) = HeaderValue::from_str(id.as_str()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, v);
    }
    response
}
