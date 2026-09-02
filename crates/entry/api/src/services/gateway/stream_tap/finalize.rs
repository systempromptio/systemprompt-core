//! Terminal accounting for a tapped stream: classifies how the stream ended
//! and spawns the audit, quota, and response-scan completion work.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::super::audit::GatewayAudit;
use super::super::quota;
use super::super::service::run_response_safety_scan;
use super::super::signature_cache::ThoughtSignatureCache;
use super::TapFinalizeCtx;
use super::accumulator::Summary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeDecision {
    Fail(&'static str),
    Complete { cost_capture_miss: bool },
}

pub const fn classify(
    error: Option<&str>,
    saw_stop: bool,
    has_content: bool,
    has_usage: bool,
) -> FinalizeDecision {
    if error.is_some() {
        return FinalizeDecision::Fail("upstream stream error");
    }
    if !saw_stop {
        return FinalizeDecision::Fail(if has_content {
            "stream ended without stop event"
        } else {
            "empty upstream stream"
        });
    }
    FinalizeDecision::Complete {
        cost_capture_miss: has_content && !has_usage,
    }
}

pub(super) fn finalize(
    audit: Arc<GatewayAudit>,
    summary: Summary,
    ctx: TapFinalizeCtx,
    origin: &'static str,
) {
    tokio::spawn(async move {
        match &audit.ctx.gateway_conversation_id {
            Some(conversation) => {
                ctx.repos
                    .thought_signatures
                    .store_from_response(conversation, &summary.response)
                    .await;
            },
            None => {
                ThoughtSignatureCache::note_uncacheable_response(
                    &summary.response,
                    "no_conversation_id",
                );
            },
        }
        if let Some(model) = summary.served_model.as_deref() {
            audit.set_served_model(model).await;
        }

        let has_content = !summary.final_bytes.is_empty();
        let has_usage = summary.saw_usage_delta
            && (summary.usage.input_tokens > 0 || summary.usage.output_tokens > 0);
        match classify(
            summary.error.as_deref(),
            summary.saw_stop,
            has_content,
            has_usage,
        ) {
            FinalizeDecision::Fail(reason) => {
                let msg = summary.error.as_deref().unwrap_or(reason);
                if let Err(e) = audit.fail(msg).await {
                    tracing::warn!(origin, error = %e, "stream audit fail failed");
                }
            },
            FinalizeDecision::Complete { cost_capture_miss } => {
                if cost_capture_miss {
                    tracing::warn!(
                        origin,
                        "stream completed with content but zero usage: cost capture miss"
                    );
                }
                let cost_microdollars = match audit
                    .complete(
                        summary.usage,
                        summary.tool_calls,
                        &summary.response,
                        &summary.final_bytes,
                    )
                    .await
                {
                    Ok(cost) => cost,
                    Err(e) => {
                        tracing::warn!(origin, error = %e, "stream audit complete failed");
                        0
                    },
                };
                quota::post_update_tokens(
                    &ctx.db,
                    &ctx.repos.quota_buckets,
                    quota::PostUpdateParams {
                        user_id: &audit.ctx.user_id,
                        windows: &ctx.policy.quota_windows,
                        input_tokens: summary.usage.input_tokens,
                        output_tokens: summary.usage.output_tokens,
                        cost_microdollars,
                    },
                )
                .await;
                run_response_safety_scan(
                    &ctx.repos.safety_findings,
                    &ctx.ai_request_id,
                    &summary.response,
                    &ctx.policy.safety,
                )
                .await;
            },
        }
    });
}
