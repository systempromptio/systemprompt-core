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

/// Rendered to the caller when the upstream stream ends with no terminal
/// event, so the abort is a stated failure on every inbound wire rather than a
/// silently closed connection.
pub const STREAM_ABORT_MESSAGE: &str = "upstream stream ended without a terminal event";

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
    stream_usage: bool,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    let state = Arc::new(Mutex::new(TapState::default()));
    let tapped = TappedStream {
        inner: upstream,
        state: Arc::clone(&state),
        inbound,
        request_model,
        stream_usage,
        audit,
        finalize_ctx: Some(finalize_ctx),
        message_stop_rendered: false,
        ended: false,
    };
    Body::from_stream(tapped)
}

/// Taps the byte-passthrough lane, where the caller receives the upstream
/// frames verbatim.
///
/// `inbound` is carried only to state an abort: the lane renders nothing of
/// its own, so a stream that ends with no terminal event would otherwise close
/// on the client with no frame explaining it.
pub fn tap_raw(
    upstream: BoxStream<'static, Result<Bytes, String>>,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    Body::from_stream(RawTappedStream {
        inner: upstream,
        state: Arc::new(Mutex::new(TapState::default())),
        decoder: SseDecoder::default(),
        inbound,
        audit,
        finalize_ctx: Some(finalize_ctx),
        ended: false,
    })
}

struct RawTappedStream {
    inner: BoxStream<'static, Result<Bytes, String>>,
    state: Arc<Mutex<TapState>>,
    decoder: SseDecoder,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    ended: bool,
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
        if self.ended {
            return Poll::Ready(None);
        }
        match self.inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                self.ended = true;
                let Some((summary, ctx)) = self.take_summary() else {
                    return Poll::Ready(None);
                };
                let aborted = summary.error.is_none() && !summary.saw_stop;
                finalize(Arc::clone(&self.audit), summary, ctx, "eof");
                if !aborted {
                    return Poll::Ready(None);
                }
                let event = CanonicalEvent::Error(STREAM_ABORT_MESSAGE.to_owned());
                match self.inbound.render_event(&event, "") {
                    Some(bytes) => Poll::Ready(Some(Ok(bytes))),
                    None => Poll::Ready(None),
                }
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
    // Why: the caller's own `stream_options.include_usage`; the trailing
    // usage chunk is rendered only for a caller that asked for one.
    stream_usage: bool,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    // Why: providers signal the end of a message more than once (Anthropic's
    // message_delta + message_stop, OpenAI's finish_reason chunk + [DONE]);
    // only the first may be rendered at all, by either the terminal path or
    // the plain-event fallback, or wires that emit a closing frame (chat's
    // [DONE], responses' response.completed, anthropic's message_stop) would
    // close the stream twice -- the second one carrying the weaker reason.
    message_stop_rendered: bool,
    // Why: the abort frame is emitted after the inner stream has already
    // reported EOF, so the next poll must not reach it again.
    ended: bool,
}

impl Stream for TappedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.ended {
            return Poll::Ready(None);
        }
        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    self.ended = true;
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
                    let terminal_suppressed = is_message_stop && self.message_stop_rendered;
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
                        .or_else(|| {
                            // Why: `terminal` already suppressed the second
                            // terminal render, but the plain-event fallback was
                            // not covered -- the Anthropic inbound renders
                            // MessageStop through `render_event`, so a repeat
                            // stop still reached the client as a second,
                            // weaker `message_stop` frame after the real one.
                            (!terminal_suppressed)
                                .then(|| self.inbound.render_event(&event, &self.request_model))
                                .flatten()
                        });
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

    // Why: an upstream that stops without a terminal event is audited as
    // failed, but the client only saw the socket close -- indistinguishable
    // from a hang. Each inbound wire has an error frame already; rendering one
    // here is the only thing that reaches the caller.
    fn finalize_on_eof(&mut self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        let Some((summary, ctx)) = self.take_summary() else {
            return Poll::Ready(None);
        };
        let aborted = summary.error.is_none() && !summary.saw_stop;
        let tail = (!aborted)
            .then(|| {
                self.inbound
                    .render_stream_tail(&summary.response, self.stream_usage)
            })
            .flatten();
        finalize(Arc::clone(&self.audit), summary, ctx, "eof");
        if !aborted {
            return Poll::Ready(tail.map(Ok));
        }
        let event = CanonicalEvent::Error(STREAM_ABORT_MESSAGE.to_owned());
        match self.inbound.render_event(&event, &self.request_model) {
            Some(bytes) => Poll::Ready(Some(Ok(bytes))),
            None => Poll::Ready(None),
        }
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
