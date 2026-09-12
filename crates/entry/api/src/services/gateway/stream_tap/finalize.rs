//! Terminal accounting for a tapped stream: classifies how the stream ended
//! and spawns the audit, quota, and response-scan completion work.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::TraceId;
use systemprompt_logging::LogActor;

use crate::routes::gateway::{TerminalOutcome, log_gateway_terminal};

use super::super::audit::GatewayAudit;
use super::super::quota;
use super::super::service::finalize::record_accounting_outcome;
use super::super::service::run_response_safety_scan;
use super::super::signature_cache::ThoughtSignatureCache;
use super::TapFinalizeCtx;
use super::accumulator::Summary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeDecision {
    Fail(FailCause),
    Complete { cost_capture_miss: bool },
}

/// Why a tapped stream ended without a usable response.
///
/// All of these land as `failed` in the audit row, so they are separated
/// here: `Upstream` is the provider failing mid-stream, `Truncated` is the
/// stream stopping without a terminal event. `client_gone` says who stopped
/// it — the consumer dropped the body before the stop event (the client hung
/// up, status 499) or upstream closed the stream early (status 502). The two
/// used to share one message, so an ordinary client abort read as an
/// upstream defect in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailCause {
    Upstream,
    Truncated {
        has_content: bool,
        client_gone: bool,
    },
}

impl FailCause {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Upstream => "upstream stream error",
            Self::Truncated {
                client_gone: true, ..
            } => "client disconnected before stop event",
            Self::Truncated {
                has_content: true,
                client_gone: false,
            } => "upstream stream ended without stop event",
            Self::Truncated {
                has_content: false,
                client_gone: false,
            } => "empty upstream stream",
        }
    }

    // Why: SSE errors arrive after the upstream HTTP status has already been sent.
    const fn status(self) -> u16 {
        match self {
            Self::Upstream
            | Self::Truncated {
                client_gone: false, ..
            } => 502,
            Self::Truncated {
                client_gone: true, ..
            } => 499,
        }
    }
}

pub const fn classify(
    error: Option<&str>,
    saw_stop: bool,
    has_content: bool,
    has_usage: bool,
    client_gone: bool,
) -> FinalizeDecision {
    if error.is_some() {
        return FinalizeDecision::Fail(FailCause::Upstream);
    }
    if !saw_stop {
        return FinalizeDecision::Fail(FailCause::Truncated {
            has_content,
            client_gone,
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
    audit.mark_upstream_end();
    tokio::spawn(async move {
        capture_signatures(&ctx, &audit, &summary).await;
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
            origin == "drop",
        ) {
            FinalizeDecision::Fail(cause) => {
                let msg = summary.error.as_deref().unwrap_or_else(|| cause.reason());
                if let Err(e) = audit.fail(msg).await {
                    tracing::warn!(origin, error = %e, "stream audit fail failed");
                }
                log_terminal(&audit, cause.status(), Some(msg));
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
                let accounting = quota::post_update_tokens(
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
                record_accounting_outcome(&audit, ctx.quota_fault_mode, accounting).await;
                run_response_safety_scan(
                    &ctx.repos.safety_findings,
                    &ctx.ai_request_id,
                    &summary.response,
                    &ctx.policy.safety,
                )
                .await;
                log_terminal(&audit, 200, None);
            },
        }
    });
}

pub(in crate::services::gateway) fn log_terminal(
    audit: &GatewayAudit,
    status: u16,
    error: Option<&str>,
) {
    let Some(access) = audit.ctx.access_log.as_ref() else {
        return;
    };
    log_gateway_terminal(TerminalOutcome {
        access,
        status,
        actor: terminal_actor(audit),
        error,
    });
}

fn terminal_actor(audit: &GatewayAudit) -> Option<LogActor> {
    if let (Some(session), Some(trace)) =
        (audit.ctx.session_id.as_ref(), audit.ctx.trace_id.as_ref())
    {
        return Some(LogActor::new(
            audit.ctx.user_id.clone(),
            session.clone(),
            trace.clone(),
        ));
    }
    LogActor::platform(TraceId::system()).ok()
}

async fn capture_signatures(ctx: &TapFinalizeCtx, audit: &GatewayAudit, summary: &Summary) {
    match &audit.ctx.gateway_conversation_id {
        Some(conversation) => {
            ctx.repos
                .thought_signatures
                .store_from_response(&audit.ctx.user_id, conversation, &summary.response)
                .await;
        },
        None => {
            ThoughtSignatureCache::note_uncacheable_response(
                &summary.response,
                "no_conversation_id",
            );
        },
    }
}
