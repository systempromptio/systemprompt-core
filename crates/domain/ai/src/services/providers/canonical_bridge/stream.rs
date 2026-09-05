//! Maps shared-codec [`CanonicalEvent`]s onto the agent's [`StreamChunk`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsageUpdate,
};

use crate::models::ai::StreamChunk;

#[must_use]
pub fn event_to_chunk(event: CanonicalEvent) -> Option<StreamChunk> {
    match event {
        CanonicalEvent::TextDelta { text, .. } => {
            (!text.is_empty()).then_some(StreamChunk::Text(text))
        },
        CanonicalEvent::UsageDelta(usage) => Some(usage_chunk(&usage)),
        CanonicalEvent::MessageStop { stop_reason, .. } => Some(finish_chunk(stop_reason)),
        _ => None,
    }
}

// Why: `tokens_used` is relayed, never recomputed here. A frame states the
// wire's own total or nothing; the stream wrapper folds the counts into one
// `CanonicalUsage` and takes `billable_total` from there, so this stays a
// single definition rather than a second, partial-frame sum.
const fn usage_chunk(usage: &CanonicalUsageUpdate) -> StreamChunk {
    StreamChunk::Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        tokens_used: usage.total_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        finish_reason: None,
    }
}

fn finish_chunk(stop_reason: Option<CanonicalStopReason>) -> StreamChunk {
    StreamChunk::Usage {
        input_tokens: None,
        output_tokens: None,
        tokens_used: None,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        reasoning_tokens: None,
        finish_reason: stop_reason.map(|s| stop_reason_str(s).to_owned()),
    }
}

const fn stop_reason_str(stop_reason: CanonicalStopReason) -> &'static str {
    match stop_reason {
        CanonicalStopReason::MaxTokens => "length",
        CanonicalStopReason::ToolUse => "tool_calls",
        CanonicalStopReason::StopSequence => "stop_sequence",
        CanonicalStopReason::EndTurn | CanonicalStopReason::Other => "stop",
    }
}
