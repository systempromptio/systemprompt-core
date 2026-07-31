//! Maps shared-codec [`CanonicalEvent`]s onto the agent's [`StreamChunk`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason, CanonicalUsage};

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

// Why: [`CanonicalUsage`] has no presence bit — an unreported field arrives as
// `0`, same as a reported zero. Every count is therefore forwarded only
// when positive, so the wrapper's last-writer-wins fold cannot let a frame
// that reports just `output_tokens` erase an `input_tokens` an earlier
// frame gave.
fn usage_chunk(usage: &CanonicalUsage) -> StreamChunk {
    let total = usage.input_tokens
        + usage.output_tokens
        + usage.cache_read_tokens
        + usage.cache_creation_tokens;
    StreamChunk::Usage {
        input_tokens: (usage.input_tokens > 0).then_some(usage.input_tokens),
        output_tokens: (usage.output_tokens > 0).then_some(usage.output_tokens),
        tokens_used: (total > 0).then_some(total),
        cache_read_tokens: (usage.cache_read_tokens > 0).then_some(usage.cache_read_tokens),
        cache_creation_tokens: (usage.cache_creation_tokens > 0)
            .then_some(usage.cache_creation_tokens),
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
