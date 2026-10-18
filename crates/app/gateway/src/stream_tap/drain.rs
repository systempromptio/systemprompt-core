//! Ending a tapped stream because the server's connection drain ran out.
//!
//! The stream records a failure carrying the partial usage it accumulated,
//! settles it like any other terminal, counts it in
//! `gateway_streams_aborted_on_shutdown_total`, and hands the client one
//! protocol-correct error frame telling it to retry, instead of a body cut
//! mid-event when the connection is dropped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex};

use bytes::Bytes;

use super::super::audit::GatewayAudit;
use super::super::protocol::canonical::CanonicalEvent;
use super::super::protocol::inbound::InboundAdapter;
use super::TapFinalizeCtx;
use super::accumulator::{TapState, extract_summary};
use super::finalize::{ClientConnection, finalize};

pub const STREAM_DRAIN_MESSAGE: &str = "server is shutting down; retry the request";
pub const STREAMS_ABORTED_ON_SHUTDOWN_TOTAL: &str = "gateway_streams_aborted_on_shutdown_total";

pub(super) struct DrainedStream<'a> {
    pub state: &'a Arc<Mutex<TapState>>,
    pub ctx: Option<TapFinalizeCtx>,
    pub audit: &'a Arc<GatewayAudit>,
    pub inbound: &'a Arc<dyn InboundAdapter>,
    pub model: &'a str,
}

pub(super) fn end(stream: DrainedStream<'_>) -> Option<Bytes> {
    metrics::counter!(STREAMS_ABORTED_ON_SHUTDOWN_TOTAL).increment(1);
    let summary = stream.state.lock().ok().and_then(|mut s| {
        if s.finalized {
            return None;
        }
        s.finalized = true;
        s.error = Some(STREAM_DRAIN_MESSAGE.to_owned());
        Some(extract_summary(&mut s))
    });
    if let (Some(summary), Some(ctx)) = (summary, stream.ctx) {
        finalize(stream.audit, summary, ctx, ClientConnection::Connected);
    }
    tracing::warn!("Gateway stream ended by the shutdown drain");
    stream.inbound.render_event(
        &CanonicalEvent::Error(STREAM_DRAIN_MESSAGE.to_owned()),
        stream.model,
    )
}
