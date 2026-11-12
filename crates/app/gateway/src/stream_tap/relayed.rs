//! Relay tap: hands the client the upstream's bytes exactly as they arrived
//! while a decoded copy of the same stream feeds the audit snapshot.
//!
//! Nothing is withheld, reordered or added: a chunk is released the moment it
//! is read, and a frame the wire decoder cannot read still reaches the client
//! and is recorded as the stream's error. The stream is cut short only when it
//! has genuinely failed: the upstream's transport broke, the retained snapshot
//! passed its cap, or the server's connection drain ran out. The decoded copy
//! is drained to its end before the audit row is settled, so the usage the
//! upstream reported last is the usage recorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::Stream;
use systemprompt_traits::StreamDrainWatch;

use super::accumulator::{Summary, TapState, accumulate_event, extract_summary};
use super::finalize::{ClientConnection, finalize};
use super::{TapFinalizeCtx, drain, lock_state};
use crate::audit::GatewayAudit;
use crate::audit::phases::StreamInFlight;
use crate::protocol::inbound::InboundAdapter;
use crate::protocol::outbound::relay::{ByteStream, EventStream};

pub fn tap_relayed(
    bytes: ByteStream,
    events: EventStream,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: TapFinalizeCtx,
) -> Body {
    Body::from_stream(RelayedStream {
        drain: audit.background().stream_drain().watch(),
        bytes: Some(bytes),
        events: Some(events),
        state: Arc::new(Mutex::new(TapState::forwarding())),
        inbound,
        audit,
        finalize_ctx: Some(finalize_ctx),
        ended: false,
        _in_flight: StreamInFlight::enter(),
    })
}

struct RelayedStream {
    drain: StreamDrainWatch,
    bytes: Option<ByteStream>,
    events: Option<EventStream>,
    state: Arc<Mutex<TapState>>,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    finalize_ctx: Option<TapFinalizeCtx>,
    ended: bool,
    _in_flight: StreamInFlight,
}

impl RelayedStream {
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

    fn absorb_events(&mut self, cx: &mut Context<'_>) {
        while let Some(events) = self.events.as_mut() {
            match events.as_mut().poll_next(cx) {
                Poll::Pending => return,
                Poll::Ready(None) => self.events = None,
                Poll::Ready(Some(Ok(event))) => {
                    accumulate_event(&mut lock_state(&self.state), &event);
                },
                Poll::Ready(Some(Err(error))) => {
                    lock_state(&self.state)
                        .error
                        .get_or_insert_with(|| error.to_string());
                },
            }
        }
    }

    fn settle(&mut self, connection: ClientConnection) {
        self.ended = true;
        if let Some((summary, ctx)) = self.take_summary() {
            finalize(&self.audit, summary, ctx, connection);
        }
    }
}

impl Stream for RelayedStream {
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
        self.absorb_events(cx);
        let Some(bytes) = self.bytes.as_mut() else {
            if self.events.is_some() {
                return Poll::Pending;
            }
            self.settle(ClientConnection::Connected);
            return Poll::Ready(None);
        };
        match bytes.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                self.bytes = None;
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            Poll::Ready(Some(Err(error))) => {
                self.bytes = None;
                lock_state(&self.state).error = Some(error.to_string());
                self.settle(ClientConnection::Connected);
                Poll::Ready(Some(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    error,
                ))))
            },
            Poll::Ready(Some(Ok(chunk))) => {
                let mut s = lock_state(&self.state);
                let within_cap = s.budget.admit_bytes(chunk.len());
                if within_cap {
                    s.final_bytes.push(&chunk);
                } else {
                    s.error
                        .get_or_insert_with(|| super::budget::ACCUMULATION_CAP_MESSAGE.to_owned());
                }
                drop(s);
                if !within_cap {
                    self.bytes = None;
                    self.settle(ClientConnection::Connected);
                    return Poll::Ready(Some(Err(std::io::Error::other(
                        super::budget::ACCUMULATION_CAP_MESSAGE,
                    ))));
                }
                Poll::Ready(Some(Ok(chunk)))
            },
        }
    }
}

impl Drop for RelayedStream {
    fn drop(&mut self) {
        let Some((summary, ctx)) = self.take_summary() else {
            return;
        };
        finalize(&self.audit, summary, ctx, ClientConnection::Disconnected);
    }
}
