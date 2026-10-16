//! Streaming response tap: re-renders upstream canonical events to the inbound
//! wire format while accumulating a full response snapshot for the audit sink.
//!
//! A tapped stream also watches the process's stream drain and, when the
//! server's connection drain runs out, ends itself with a terminal error frame
//! (see [`drain`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod abort;
pub mod accumulator;
mod drain;
mod finalize;
mod raw;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::{BoxStream, Stream};
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_traits::StreamDrainWatch;
use systemprompt_wire::error::WireStreamError;

use self::accumulator::{Summary, TapState, accumulate_event, extract_summary, snapshot};
use self::finalize::finalize;
use super::audit::GatewayAudit;
use super::audit::phases::StreamInFlight;
use super::protocol::canonical::CanonicalEvent;
use super::protocol::inbound::InboundAdapter;
use crate::policies::GatewayPolicySpec;

pub(super) use self::finalize::log_terminal;
pub use self::finalize::{ClientConnection, FailCause, FinalizeDecision, classify};

pub use self::abort::STREAM_ABORT_MESSAGE;
pub use self::drain::{STREAM_DRAIN_MESSAGE, STREAMS_ABORTED_ON_SHUTDOWN_TOTAL};
pub use self::raw::tap_raw;

/// Shared by the streaming and buffered completion tasks so both debit quota
/// and run the response-phase safety scan identically.
#[derive(Debug)]
pub struct TapFinalizeCtx {
    pub db: DbPool,
    pub repos: crate::GatewayRepositories,
    pub policy: GatewayPolicySpec,
    pub quota_fault_mode: QuotaFaultMode,
    pub ai_request_id: AiRequestId,
}

/// How the tapped stream is rendered back to the caller.
///
/// `stream_usage` is the caller's own `stream_options.include_usage`; it
/// decides whether the closing frames carry a usage chunk.
#[derive(Debug)]
pub struct TapRender {
    pub inbound: Arc<dyn InboundAdapter>,
    pub request_model: String,
    pub stream_usage: bool,
}

pub fn tap(
    upstream: BoxStream<'static, Result<CanonicalEvent, WireStreamError>>,
    render: TapRender,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    let TapRender {
        inbound,
        request_model,
        stream_usage,
    } = render;
    let state = Arc::new(Mutex::new(TapState::default()));
    let tapped = TappedStream {
        drain: audit.background().stream_drain().watch(),
        inner: upstream,
        state: Arc::clone(&state),
        inbound,
        request_model,
        stream_usage,
        audit,
        finalize_ctx: Some(finalize_ctx),
        message_stop_rendered: false,
        ended: false,
        _in_flight: StreamInFlight::enter(),
    };
    Body::from_stream(tapped)
}

struct TappedStream {
    drain: StreamDrainWatch,
    inner: BoxStream<'static, Result<CanonicalEvent, WireStreamError>>,
    state: Arc<Mutex<TapState>>,
    inbound: Arc<dyn InboundAdapter>,
    request_model: String,
    stream_usage: bool,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    message_stop_rendered: bool,
    ended: bool,
    _in_flight: StreamInFlight,
}

impl Stream for TappedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.ended {
            return Poll::Ready(None);
        }
        if self.drain.poll_ended(cx) {
            self.ended = true;
            let ctx = self.finalize_ctx.take();
            return Poll::Ready(
                drain::end(drain::DrainedStream {
                    state: &self.state,
                    ctx,
                    audit: &self.audit,
                    inbound: &self.inbound,
                    model: &self.request_model,
                })
                .map(Ok),
            );
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
                        s.error = Some(e.to_string());
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

    fn finalize_on_eof(&mut self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        let Some((summary, ctx)) = self.take_summary() else {
            return Poll::Ready(None);
        };
        let aborted = abort::is_abort(&summary);
        let tail = (!aborted)
            .then(|| abort::tail_frames(&self.inbound, &summary.response, self.stream_usage))
            .flatten();
        finalize(&self.audit, summary, ctx, ClientConnection::Connected);
        if !aborted {
            return Poll::Ready(tail.map(Ok));
        }
        Poll::Ready(abort::abort_frame(&self.inbound, &self.request_model).map(Ok))
    }
}

impl Drop for TappedStream {
    fn drop(&mut self) {
        let Some((summary, ctx)) = self.take_summary() else {
            return;
        };
        finalize(&self.audit, summary, ctx, ClientConnection::Disconnected);
    }
}
