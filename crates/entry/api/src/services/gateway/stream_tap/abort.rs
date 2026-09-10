//! Terminal frames for completed and interrupted gateway streams.
//!
//! Uses the inbound adapter to render protocol-specific error, usage and
//! completion frames. A stream without a terminal event is classified as
//! aborted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use bytes::Bytes;

use super::super::protocol::canonical_response::{CanonicalEvent, CanonicalResponse};
use super::super::protocol::inbound::InboundAdapter;
use super::accumulator::Summary;

pub const STREAM_ABORT_MESSAGE: &str = "upstream stream ended without a terminal event";

pub(super) const fn is_abort(summary: &Summary) -> bool {
    summary.error.is_none() && !summary.saw_stop
}

pub(super) fn abort_frame(inbound: &Arc<dyn InboundAdapter>, model: &str) -> Option<Bytes> {
    let event = CanonicalEvent::Error(STREAM_ABORT_MESSAGE.to_owned());
    inbound.render_event(&event, model)
}

pub(super) fn tail_frames(
    inbound: &Arc<dyn InboundAdapter>,
    response: &CanonicalResponse,
    stream_usage: bool,
) -> Option<Bytes> {
    inbound.render_stream_tail(response, stream_usage)
}
