//! What the caller is told when a tapped stream ends.
//!
//! Two endings, and both used to be silence. A stream that stopped with no
//! terminal event was audited as failed while the client saw only a closed
//! socket, indistinguishable from a hang; a stream that ended cleanly on the
//! Chat Completions surface still owed the caller its usage chunk and the
//! `[DONE]` sentinel. Both are rendered here, through the inbound adapter, so
//! each surface states them in its own wire's vocabulary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use bytes::Bytes;

use super::super::protocol::canonical_response::{CanonicalEvent, CanonicalResponse};
use super::super::protocol::inbound::InboundAdapter;
use super::accumulator::Summary;

// Why: the message names the missing signal rather than the symptom, because
// it is what a human reading a client log has to act on, and it matches the
// reason the audit row carries for the same turn.
pub const STREAM_ABORT_MESSAGE: &str = "upstream stream ended without a terminal event";

// Why: an error the upstream already stated is relayed as itself; the abort
// frame is only for a stream that stated nothing at all.
pub(super) const fn is_abort(summary: &Summary) -> bool {
    summary.error.is_none() && !summary.saw_stop
}

// Why: the abort reaches the caller as an ordinary canonical error, so each
// inbound surface renders the frame its own clients read -- `event: error` on
// Anthropic, an error chunk on Chat Completions, `response.failed` on the
// Responses wire.
pub(super) fn abort_frame(inbound: &Arc<dyn InboundAdapter>, model: &str) -> Option<Bytes> {
    let event = CanonicalEvent::Error(STREAM_ABORT_MESSAGE.to_owned());
    inbound.render_event(&event, model)
}

// Why: the closing frames of a turn that ended properly. Only Chat
// Completions renders any, and the counts come from the accumulated snapshot
// because they arrive after the finish chunk on that wire.
pub(super) fn tail_frames(
    inbound: &Arc<dyn InboundAdapter>,
    response: &CanonicalResponse,
    stream_usage: bool,
) -> Option<Bytes> {
    inbound.render_stream_tail(response, stream_usage)
}
