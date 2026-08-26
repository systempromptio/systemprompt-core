//! Streaming response tap: re-renders upstream canonical events to the inbound
//! wire format while accumulating a full response snapshot for the audit sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod accumulator;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::accumulator::{Summary, TapState, accumulate_event, extract_summary, snapshot};
}

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::{BoxStream, Stream};
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;

use self::accumulator::{Summary, TapState, accumulate_event, extract_summary, snapshot};
use super::audit::GatewayAudit;
use super::policy::GatewayPolicySpec;
use super::protocol::canonical_response::CanonicalEvent;
use super::protocol::inbound::InboundAdapter;
use super::protocol::outbound::anthropic::streaming::SseDecoder;
use super::quota;
use super::service::run_response_safety_scan;
use super::signature_cache::ThoughtSignatureCache;

/// Shared by the streaming and buffered completion tasks so both debit quota
/// and run the response-phase safety scan identically.
#[derive(Debug)]
pub struct TapFinalizeCtx {
    pub db: DbPool,
    pub repos: crate::services::gateway::GatewayRepositories,
    pub policy: GatewayPolicySpec,
    pub ai_request_id: AiRequestId,
}

pub fn tap(
    upstream: BoxStream<'static, Result<CanonicalEvent, String>>,
    inbound: Arc<dyn InboundAdapter>,
    request_model: String,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    let state = Arc::new(Mutex::new(TapState::default()));
    let tapped = TappedStream {
        inner: upstream,
        state: Arc::clone(&state),
        inbound,
        request_model,
        audit,
        finalize_ctx: Some(finalize_ctx),
        message_stop_rendered: false,
    };
    Body::from_stream(tapped)
}

pub fn tap_raw(
    upstream: BoxStream<'static, Result<Bytes, String>>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    Body::from_stream(RawTappedStream {
        inner: upstream,
        state: Arc::new(Mutex::new(TapState::default())),
        decoder: SseDecoder::default(),
        audit,
        finalize_ctx: Some(finalize_ctx),
    })
}

struct RawTappedStream {
    inner: BoxStream<'static, Result<Bytes, String>>,
    state: Arc<Mutex<TapState>>,
    decoder: SseDecoder,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
}

impl RawTappedStream {
    fn take_summary(&mut self) -> Option<(Summary, TapFinalizeCtx)> {
        let ctx = self.finalize_ctx.take()?;
        self.state.lock().ok().and_then(|mut s| {
            if s.finalized {
                return None;
            }
            s.finalized = true;
            Some((extract_summary(&mut s), ctx))
        })
    }
}

impl Stream for RawTappedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                if let Some((summary, ctx)) = self.take_summary() {
                    finalize(Arc::clone(&self.audit), summary, ctx, "eof");
                }
                Poll::Ready(None)
            },
            Poll::Ready(Some(Err(e))) => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some(e.clone());
                }
                Poll::Ready(Some(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    e,
                ))))
            },
            Poll::Ready(Some(Ok(bytes))) => {
                let events = self.decoder.push(&bytes);
                if let Ok(mut s) = self.state.lock() {
                    for event in &events {
                        accumulate_event(&mut s, event);
                    }
                    s.final_bytes.extend_from_slice(&bytes);
                }
                Poll::Ready(Some(Ok(bytes)))
            },
        }
    }
}

impl Drop for RawTappedStream {
    fn drop(&mut self) {
        let Some((summary, ctx)) = self.take_summary() else {
            return;
        };
        finalize(Arc::clone(&self.audit), summary, ctx, "drop");
    }
}

struct TappedStream {
    inner: BoxStream<'static, Result<CanonicalEvent, String>>,
    state: Arc<Mutex<TapState>>,
    inbound: Arc<dyn InboundAdapter>,
    request_model: String,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    // Why: providers signal the end of a message more than once (Anthropic's
    // message_delta + message_stop, OpenAI's finish_reason chunk + [DONE]);
    // only the first may drive the adapter's terminal render or wires that
    // emit a closing frame (chat's [DONE], responses' response.completed)
    // would close the stream twice.
    message_stop_rendered: bool,
}

impl Stream for TappedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    return self.finalize_on_eof();
                },
                Poll::Ready(Some(Err(e))) => {
                    if let Ok(mut s) = self.state.lock() {
                        s.error = Some(e.clone());
                    }
                    let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, e);
                    return Poll::Ready(Some(Err(err)));
                },
                Poll::Ready(Some(Ok(event))) => {
                    let is_message_stop = matches!(event, CanonicalEvent::MessageStop { .. });
                    let terminal = matches!(event, CanonicalEvent::ContentBlockStop { .. })
                        || (is_message_stop && !self.message_stop_rendered);
                    let snap = self.state.lock().map_or(None, |mut s| {
                        accumulate_event(&mut s, &event);
                        terminal.then(|| snapshot(&s))
                    });
                    if is_message_stop {
                        self.message_stop_rendered = true;
                    }
                    let rendered = snap
                        .as_ref()
                        .and_then(|snapshot| {
                            self.inbound.render_terminal_event(
                                &event,
                                snapshot,
                                &self.request_model,
                            )
                        })
                        .or_else(|| self.inbound.render_event(&event, &self.request_model));
                    if let Some(bytes) = rendered {
                        if let Ok(mut s) = self.state.lock() {
                            s.final_bytes.extend_from_slice(&bytes);
                        }
                        return Poll::Ready(Some(Ok(bytes)));
                    }
                },
            }
        }
    }
}

impl TappedStream {
    fn take_summary(&mut self) -> Option<(Summary, TapFinalizeCtx)> {
        let ctx = self.finalize_ctx.take()?;
        self.state.lock().ok().and_then(|mut s| {
            if s.finalized {
                return None;
            }
            s.finalized = true;
            Some((extract_summary(&mut s), ctx))
        })
    }

    fn finalize_on_eof(&mut self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        let Some((summary, ctx)) = self.take_summary() else {
            return Poll::Ready(None);
        };
        finalize(Arc::clone(&self.audit), summary, ctx, "eof");
        Poll::Ready(None)
    }
}

impl Drop for TappedStream {
    fn drop(&mut self) {
        let Some((summary, ctx)) = self.take_summary() else {
            return;
        };
        finalize(Arc::clone(&self.audit), summary, ctx, "drop");
    }
}

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

fn finalize(audit: Arc<GatewayAudit>, summary: Summary, ctx: TapFinalizeCtx, origin: &'static str) {
    match &audit.ctx.gateway_conversation_id {
        Some(conversation) => {
            ThoughtSignatureCache::global().store_from_response(conversation, &summary.response);
        },
        None => {
            ThoughtSignatureCache::note_uncacheable_response(
                &summary.response,
                "no_conversation_id",
            );
        },
    }
    tokio::spawn(async move {
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
