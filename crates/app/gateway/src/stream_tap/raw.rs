//! Pass-through tap: forwards upstream SSE bytes untouched while decoding
//! them for the audit snapshot.
//!
//! What the client receives is exactly what the tap accounted for. Every
//! frame is decoded before it is released; a data frame that cannot be read
//! as an Anthropic envelope is withheld and counted on the audit record, and
//! every released byte — comments and field-only frames included — counts
//! toward the retained payload and the accumulation cap. A stream that ends
//! with withheld frames and no decodable one ends as a protocol failure.
//!
//! Bytes are released only up to the last complete SSE frame, so a terminal
//! frame the tap appends (shutdown drain, abort) never lands inside a frame
//! the upstream split across chunks. A frame that grows past the framer's cap
//! ends the stream as a failure: the client gets an error frame and the audit
//! row is failed.
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
use systemprompt_wire::anthropic::{DecodedFrame, SseFrameDecoder as SseDecoder};
use systemprompt_wire::error::WireStreamError;
use systemprompt_wire::sse::{SseFrameTooLarge, SseFramer};

use super::accumulator::{Summary, TapState, accumulate_event, extract_summary};
use super::finalize::{ClientConnection, finalize};
use super::{TapFinalizeCtx, abort, drain, lock_state};
use crate::audit::GatewayAudit;
use crate::audit::phases::StreamInFlight;
use crate::protocol::canonical::CanonicalEvent;
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
        state: Arc::new(Mutex::new(TapState::forwarding())),
        decoder: SseDecoder::default(),
        framer: SseFramer::default(),
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
    framer: SseFramer,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    ended: bool,
    _in_flight: StreamInFlight,
}

impl RawTappedStream {
    fn take_summary(&mut self) -> Option<(Summary, TapFinalizeCtx)> {
        let ctx = self.finalize_ctx.take()?;
        let mut s = lock_state(&self.state);
        if s.finalized {
            return None;
        }
        s.finalized = true;
        let summary = extract_summary(&mut s);
        drop(s);
        Some((summary, ctx))
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
                Poll::Ready(Some(Err(WireStreamError::FrameTooLarge(error)))) => {
                    lock_state(&self.state).error = Some(error.to_string());
                    return self.end_failed(error.to_string(), Vec::new());
                },
                Poll::Ready(Some(Err(e))) => {
                    lock_state(&self.state).error = Some(e.to_string());
                    return Poll::Ready(Some(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        e,
                    ))));
                },
                Poll::Ready(Some(Ok(bytes))) => match self.absorb(&bytes) {
                    Absorbed::Frames(complete) => return Poll::Ready(Some(Ok(complete))),
                    Absorbed::FrameTooLarge { complete, error } => {
                        return self.end_failed(error.to_string(), complete);
                    },
                    Absorbed::OverCap(complete) => {
                        return self.end_failed(
                            super::budget::ACCUMULATION_CAP_MESSAGE.to_owned(),
                            complete,
                        );
                    },
                    Absorbed::Held => {},
                },
            }
        }
    }
}

enum Absorbed {
    Frames(Bytes),
    Held,
    OverCap(Vec<u8>),
    FrameTooLarge {
        complete: Vec<u8>,
        error: SseFrameTooLarge,
    },
}

impl RawTappedStream {
    fn absorb(&mut self, bytes: &Bytes) -> Absorbed {
        let mut complete: Vec<u8> = Vec::new();
        for frame in self.framer.push_raw(bytes) {
            let frame = match frame {
                Ok(frame) => frame,
                Err(error) => {
                    lock_state(&self.state).error = Some(error.to_string());
                    return Absorbed::FrameTooLarge { complete, error };
                },
            };
            let events = match frame.data.as_deref().map(|d| self.decoder.decode_data(d)) {
                None | Some(DecodedFrame::Benign) => Vec::new(),
                Some(DecodedFrame::Events(events)) => events,
                Some(DecodedFrame::Dropped) => {
                    lock_state(&self.state).note_dropped_frame();
                    continue;
                },
            };
            let mut s = lock_state(&self.state);
            for event in &events {
                accumulate_event(&mut s, event);
            }
            if !s.budget.admit_bytes(frame.raw.len()) {
                s.error
                    .get_or_insert_with(|| super::budget::ACCUMULATION_CAP_MESSAGE.to_owned());
                return Absorbed::OverCap(complete);
            }
            s.final_bytes.push(&frame.raw);
            drop(s);
            complete.extend_from_slice(&frame.raw);
        }
        if complete.is_empty() {
            Absorbed::Held
        } else {
            Absorbed::Frames(Bytes::from(complete))
        }
    }

    fn end_failed(
        &mut self,
        message: String,
        mut complete: Vec<u8>,
    ) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        self.ended = true;
        if let Some((summary, ctx)) = self.take_summary() {
            finalize(&self.audit, summary, ctx, ClientConnection::Connected);
        }
        if let Some(frame) = self
            .inbound
            .render_event(&CanonicalEvent::Error(message), "")
        {
            complete.extend_from_slice(&frame);
        }
        Poll::Ready((!complete.is_empty()).then(|| Ok(Bytes::from(complete))))
    }

    fn finish(&mut self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        self.ended = true;
        if !self.framer.remainder().trim_ascii().is_empty() {
            lock_state(&self.state).note_dropped_frame();
        }
        if let Some(error) = self.decoder.finish() {
            lock_state(&self.state).error = Some(error.to_string());
            return self.end_failed(error.to_string(), Vec::new());
        }
        let Some((summary, ctx)) = self.take_summary() else {
            return Poll::Ready(None);
        };
        let aborted = abort::is_abort(&summary);
        finalize(&self.audit, summary, ctx, ClientConnection::Connected);
        if !aborted {
            return Poll::Ready(None);
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
