//! Re-indexing tool-call events by tool ordinal for wires that count tool
//! calls apart from text and thinking blocks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::protocol::canonical::{CanonicalEvent, ContentBlockKind};
use super::accumulator::{BlockAccumulator, TapState};

// Why: OpenAI Chat clients index `tool_calls` from 0 per choice; the canonical
// index counts thinking and text blocks too.
pub fn with_tool_ordinal(state: &TapState, event: &CanonicalEvent) -> Option<CanonicalEvent> {
    let ordinal = |index: u32| {
        state
            .blocks
            .iter()
            .take(index as usize)
            .filter(|b| matches!(b, BlockAccumulator::ToolUse { .. }))
            .count() as u32
    };
    match event {
        CanonicalEvent::ContentBlockStart {
            index,
            block: block @ ContentBlockKind::ToolUse { .. },
        } => Some(CanonicalEvent::ContentBlockStart {
            index: ordinal(*index),
            block: block.clone(),
        }),
        CanonicalEvent::ToolUseDelta {
            index,
            partial_json,
        } => Some(CanonicalEvent::ToolUseDelta {
            index: ordinal(*index),
            partial_json: partial_json.clone(),
        }),
        _ => None,
    }
}
