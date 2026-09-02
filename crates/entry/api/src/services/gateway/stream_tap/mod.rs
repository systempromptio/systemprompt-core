//! Streaming response tap: re-renders upstream canonical events to the inbound
//! wire format while accumulating a full response snapshot for the audit sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod accumulator;
mod finalize;

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
use self::finalize::finalize;
use super::audit::GatewayAudit;
use super::policy::GatewayPolicySpec;
use super::protocol::canonical_response::CanonicalEvent;
use super::protocol::inbound::InboundAdapter;
use super::protocol::outbound::anthropic::streaming::SseDecoder;

pub use self::finalize::{FailCause, FinalizeDecision, classify};

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
