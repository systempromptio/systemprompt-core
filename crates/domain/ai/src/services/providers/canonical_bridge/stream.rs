//! Projects shared-codec [`CanonicalEvent`]s onto the agent's text
//! [`StreamChunk`]s and finish-reason strings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_wire::canonical::{CanonicalEvent, CanonicalStopReason};

use crate::models::ai::StreamChunk;

#[must_use]
pub fn event_to_chunk(event: CanonicalEvent) -> Option<StreamChunk> {
    match event {
        CanonicalEvent::TextDelta { text, .. } => {
            (!text.is_empty()).then_some(StreamChunk::Text(text))
        },
        _ => None,
    }
}

#[must_use]
pub const fn finish_reason(stop_reason: CanonicalStopReason) -> &'static str {
    match stop_reason {
        CanonicalStopReason::MaxTokens => "length",
        CanonicalStopReason::ToolUse => "tool_calls",
        CanonicalStopReason::StopSequence => "stop_sequence",
        CanonicalStopReason::Refusal => "content_filter",
        CanonicalStopReason::EndTurn | CanonicalStopReason::Other => "stop",
    }
}
