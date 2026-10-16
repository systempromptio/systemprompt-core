//! Pass-through tap: forwards upstream SSE bytes untouched while decoding
//! them for the audit snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::{BoxStream, Stream};
use systemprompt_traits::StreamDrainWatch;
use systemprompt_wire::anthropic::SseFrameDecoder as SseDecoder;
use systemprompt_wire::error::WireStreamError;

use super::accumulator::{Summary, TapState, accumulate_event, extract_summary};
use super::finalize::{ClientConnection, finalize};
use super::{TapFinalizeCtx, abort, drain};
use crate::audit::GatewayAudit;
use crate::audit::phases::StreamInFlight;
use crate::protocol::inbound::InboundAdapter;

pub fn tap_raw(
    upstream: BoxStream<'static, Result<Bytes, WireStreamError>>,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    Body::from_stream(RawTappedStream {
        drain: audit.background().stream_drain().watch(),
        inner: upstream,
        state: Arc::new(Mutex::new(TapState::default())),
        decoder: SseDecoder::default(),
        inbound,
        audit,
        finalize_ctx: Some(finalize_ctx),
        ended: false,
        _in_flight: StreamInFlight::enter(),
    })
}

struct RawTappedStream {
    drain: StreamDrainWatch,
    inner: BoxStream<'static, Result<Bytes, WireStreamError>>,
    state: Arc<Mutex<TapState>>,
    decoder: SseDecoder,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    ended: bool,
    _in_flight: StreamInFlight,
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
        if self.drain.poll_ended(cx) {
            self.ended = true;
            let ctx = self.finalize_ctx.take();
            return Poll::Ready(
                drain::end(drain::DrainedStream {
                    state: &self.state,
                    ctx,
                    audit: &self.audit,
                    inbound: &self.inbound,
                    model: "",
                })
                .map(Ok),
            );
        }
        match self.inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                self.ended = true;
                let Some((summary, ctx)) = self.take_summary() else {
                    return Poll::Ready(None);
                };
                let aborted = abort::is_abort(&summary);
                finalize(&self.audit, summary, ctx, ClientConnection::Connected);
                if !aborted {
                    return Poll::Ready(None);
                }
                Poll::Ready(abort::abort_frame(&self.inbound, "").map(Ok))
            },
            Poll::Ready(Some(Err(e))) => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some(e.to_string());
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
        finalize(&self.audit, summary, ctx, ClientConnection::Disconnected);
    }
}
