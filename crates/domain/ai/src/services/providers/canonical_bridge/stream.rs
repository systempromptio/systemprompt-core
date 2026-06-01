//! Maps shared-codec [`CanonicalEvent`]s onto the agent's [`StreamChunk`]s.

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

fn usage_chunk(usage: &CanonicalUsage) -> StreamChunk {
    StreamChunk::Usage {
        input_tokens: Some(usage.input_tokens),
        output_tokens: Some(usage.output_tokens),
        tokens_used: Some(usage.input_tokens + usage.output_tokens),
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
