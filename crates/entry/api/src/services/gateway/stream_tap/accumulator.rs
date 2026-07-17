//! Streaming-event accumulation: folds canonical SSE events into a `TapState`
//! and extracts a finalized `Summary` for the audit sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::{Bytes, BytesMut};
use systemprompt_identifiers::AiToolCallId;

use super::super::captures::{CapturedToolUse, CapturedUsage};
use super::super::protocol::canonical::CanonicalContent;
use super::super::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug, Default)]
pub struct TapState {
    response_id: String,
    served_model: String,
    usage: CanonicalUsage,
    blocks: Vec<BlockAccumulator>,
    final_stop_reason: Option<CanonicalStopReason>,
    pub(super) final_bytes: BytesMut,
    pub(super) error: Option<String>,
    pub(super) finalized: bool,
}

#[derive(Debug, Clone)]
enum BlockAccumulator {
    Text(String),
    Thinking {
        text: String,
        signature: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        partial: String,
        signature: Option<String>,
    },
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub struct Summary {
    pub usage: CapturedUsage,
    pub tool_calls: Vec<CapturedToolUse>,
    pub response: CanonicalResponse,
    pub final_bytes: Bytes,
    pub served_model: Option<String>,
    pub error: Option<String>,
    pub saw_stop: bool,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn extract_summary(state: &mut TapState) -> Summary {
    let response = build_response(state);
    let usage = CapturedUsage {
        input_tokens: state.usage.input_tokens,
        output_tokens: state.usage.output_tokens,
        cache_read_tokens: state.usage.cache_read_tokens,
        cache_creation_tokens: state.usage.cache_creation_tokens,
    };
    let tool_calls = response
        .content
        .iter()
        .filter_map(|c| {
            if let CanonicalContent::ToolUse {
                id, name, input, ..
            } = c
            {
                Some(CapturedToolUse {
                    ai_tool_call_id: AiToolCallId::new(id.clone()),
                    tool_name: name.clone(),
                    tool_input: serde_json::to_string(input).unwrap_or_else(|e| {
                        tracing::warn!(error = %e, tool = %name, "failed to serialise tool_input");
                        String::new()
                    }),
                })
            } else {
                None
            }
        })
        .collect();
    let final_bytes = std::mem::take(&mut state.final_bytes).freeze();
    let served_model = if state.served_model.is_empty() {
        None
    } else {
        Some(state.served_model.clone())
    };
    Summary {
        usage,
        tool_calls,
        response,
        final_bytes,
        served_model,
        error: state.error.clone(),
        saw_stop: state.final_stop_reason.is_some(),
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn snapshot(state: &TapState) -> CanonicalResponse {
    build_response(state)
}

fn build_response(state: &TapState) -> CanonicalResponse {
    let content = state
        .blocks
        .iter()
        .map(|b| match b {
            BlockAccumulator::Text(t) => CanonicalContent::Text(t.clone()),
            BlockAccumulator::Thinking { text, signature } => CanonicalContent::Thinking {
                text: text.clone(),
                signature: signature.clone(),
            },
            BlockAccumulator::ToolUse {
                id,
                name,
                partial,
                signature,
            } => CanonicalContent::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: serde_json::from_str(partial)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                signature: signature.clone(),
            },
        })
        .collect();
    CanonicalResponse {
        id: state.response_id.clone(),
        model: state.served_model.clone(),
        content,
        stop_reason: state.final_stop_reason,
        usage: state.usage,
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
    }
}

fn start_block(state: &mut TapState, index: u32, block: &ContentBlockKind) {
    let slot = match block {
        ContentBlockKind::Text => BlockAccumulator::Text(String::new()),
        ContentBlockKind::Thinking { signature } => BlockAccumulator::Thinking {
            text: String::new(),
            signature: signature.clone(),
        },
        ContentBlockKind::ToolUse {
            id,
            name,
            signature,
        } => BlockAccumulator::ToolUse {
            id: id.clone(),
            name: name.clone(),
            partial: String::new(),
            signature: signature.clone(),
        },
    };
    let idx = index as usize;
    while state.blocks.len() <= idx {
        state.blocks.push(BlockAccumulator::Text(String::new()));
    }
    state.blocks[idx] = slot;
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn accumulate_event(state: &mut TapState, event: &CanonicalEvent) {
    match event {
        CanonicalEvent::MessageStart { id, model, usage } => {
            state.response_id.clone_from(id);
            if !model.is_empty() {
                state.served_model.clone_from(model);
            }
            state.usage = *usage;
        },
        CanonicalEvent::ContentBlockStart { index, block } => {
            start_block(state, *index, block);
        },
        CanonicalEvent::TextDelta { index, text } => {
            if let Some(BlockAccumulator::Text(buf)) = state.blocks.get_mut(*index as usize) {
                buf.push_str(text);
            }
        },
        CanonicalEvent::ThinkingDelta { index, text } => {
            if let Some(BlockAccumulator::Thinking { text: buf, .. }) =
                state.blocks.get_mut(*index as usize)
            {
                buf.push_str(text);
            }
        },
        CanonicalEvent::SignatureDelta { index, signature } => {
            if let Some(BlockAccumulator::Thinking { signature: sig, .. }) =
                state.blocks.get_mut(*index as usize)
            {
                *sig = Some(signature.clone());
            }
        },
        CanonicalEvent::ToolUseDelta {
            index,
            partial_json,
        } => {
            if let Some(BlockAccumulator::ToolUse { partial, .. }) =
                state.blocks.get_mut(*index as usize)
            {
                partial.push_str(partial_json);
            }
        },
        CanonicalEvent::ContentBlockStop { .. } => {},
        CanonicalEvent::UsageDelta(u) => {
            if u.input_tokens > 0 {
                state.usage.input_tokens = u.input_tokens;
            }
            if u.output_tokens > 0 {
                state.usage.output_tokens = u.output_tokens;
            }
            if u.cache_read_tokens > 0 {
                state.usage.cache_read_tokens = u.cache_read_tokens;
            }
            if u.cache_creation_tokens > 0 {
                state.usage.cache_creation_tokens = u.cache_creation_tokens;
            }
        },
        CanonicalEvent::MessageStop { stop_reason, .. } => {
            state.final_stop_reason = stop_reason.or(Some(CanonicalStopReason::EndTurn));
        },
        CanonicalEvent::Error(msg) => {
            state.error = Some(msg.clone());
        },
    }
}
