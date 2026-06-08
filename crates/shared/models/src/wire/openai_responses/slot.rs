//! Per-output-item slot state machine the Responses SSE pass tracks.
//!
//! Responses streams reference each output item by `output_index`; this maps
//! those upstream indices to the canonical block index emitted downstream,
//! keyed by the kind of block the slot carries.

use crate::wire::canonical::CanonicalStopReason;

pub(super) struct ResponsesStreamState {
    pub(super) buf: Vec<u8>,
    pub(super) model: String,
    pub(super) response_id: String,
    pub(super) started: bool,
    pub(super) items: Vec<ItemSlot>,
}

pub(super) struct ItemSlot {
    pub(super) output_index: i64,
    pub(super) canonical_index: u32,
    pub(super) kind: SlotKind,
}

pub(super) enum SlotKind {
    Message,
    Function,
    Reasoning,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotKindMatch {
    Message,
    Function,
    Reasoning,
}

// Responses has no finish-reason field: tool use is signalled by a
// `function_call` output item, truncation by `incomplete_details.reason`.
pub(super) fn stop_reason(
    items: &[ItemSlot],
    incomplete_reason: Option<&str>,
) -> CanonicalStopReason {
    if items.iter().any(|s| matches!(s.kind, SlotKind::Function)) {
        return CanonicalStopReason::ToolUse;
    }
    match incomplete_reason {
        Some("max_output_tokens") => CanonicalStopReason::MaxTokens,
        Some(_) => CanonicalStopReason::Other,
        None => CanonicalStopReason::EndTurn,
    }
}

pub(super) fn lookup_canonical(
    items: &[ItemSlot],
    output_index: i64,
    want: SlotKindMatch,
) -> Option<u32> {
    items.iter().find_map(|s| {
        let kind_match = matches!(
            (&s.kind, want),
            (SlotKind::Message, SlotKindMatch::Message)
                | (SlotKind::Function, SlotKindMatch::Function)
                | (SlotKind::Reasoning, SlotKindMatch::Reasoning)
        );
        (s.output_index == output_index && kind_match).then_some(s.canonical_index)
    })
}
