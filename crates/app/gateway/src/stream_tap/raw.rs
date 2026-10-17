//! Pass-through tap: forwards upstream SSE bytes untouched while decoding
//! them for the audit snapshot.
//!
//! Bytes are released only up to the last complete SSE frame, so a terminal
//! frame the tap appends (shutdown drain, abort) never lands inside a frame
//! the upstream split across chunks.
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
use systemprompt_wire::sse::frame_end;

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
        held: Vec::new(),
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
    held: Vec<u8>,
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
        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return self.finish(),
                Poll::Ready(Some(Err(e))) => {
                    if let Ok(mut s) = self.state.lock() {
                        s.error = Some(e.to_string());
                    }
                    return Poll::Ready(Some(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        e,
                    ))));
                },
                Poll::Ready(Some(Ok(bytes))) => {
                    if let Some(complete) = self.absorb(&bytes) {
                        return Poll::Ready(Some(Ok(complete)));
                    }
                },
            }
        }
    }
}

impl RawTappedStream {
    fn absorb(&mut self, bytes: &Bytes) -> Option<Bytes> {
        let events = self.decoder.push(bytes);
        if let Ok(mut s) = self.state.lock() {
            for event in &events {
                accumulate_event(&mut s, event);
            }
            s.final_bytes.extend_from_slice(bytes);
        }
        self.held.extend_from_slice(bytes);
        let mut cut = 0;
        while let Some(end) = frame_end(&self.held[cut..]) {
            cut += end;
        }
        (cut > 0).then(|| Bytes::from(self.held.drain(..cut).collect::<Vec<u8>>()))
    }

    fn finish(&mut self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        self.ended = true;
        let rest = (!self.held.is_empty()).then(|| Bytes::from(std::mem::take(&mut self.held)));
        let Some((summary, ctx)) = self.take_summary() else {
            return Poll::Ready(rest.map(Ok));
        };
        let aborted = abort::is_abort(&summary);
        finalize(&self.audit, summary, ctx, ClientConnection::Connected);
        if !aborted {
            return Poll::Ready(rest.map(Ok));
        }
        Poll::Ready(abort::abort_frame(&self.inbound, "").map(Ok))
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
