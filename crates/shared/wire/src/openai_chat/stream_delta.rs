//! Per-delta handling for the Chat Completions SSE stream, and the state it
//! threads through.
//!
//! A chunk's `delta` carries reasoning, answer text and tool-call fragments on
//! three independent tracks; each opens its own canonical content block the
//! first time it appears and keeps that index for the rest of the turn.
//! Indices are handed out in arrival order, so a reasoning block that comes
//! first takes index 0 and the answer text follows it -- and reasoning is
//! closed the moment answer text or a tool call begins, because a thinking
//! block left open while text streams renders as one interleaved block on the
//! inbound Anthropic surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::Value;
use systemprompt_identifiers::MessageId;

use crate::canonical::{CanonicalEvent, ContentBlockKind};
use crate::error::WireStreamError;
use crate::sse::SseFramer;

pub(super) struct OpenAiChatStreamState {
    pub(super) framer: SseFramer,
    pub(super) model: String,
    pub(super) message_id: Option<MessageId>,
    pub(super) started: bool,
    pub(super) text_block: Option<u32>,
    pub(super) next_index: u32,
    pub(super) tool_calls: Vec<ToolCallProgress>,
    pub(super) reasoning_block: Option<u32>,
    pub(super) saw_tool_call: bool,
    pub(super) stopped: bool,
    pub(super) pending_finish: Option<String>,
}

pub(super) struct ToolCallProgress {
    pub(super) index: u32,
    pub(super) provider_index: Option<i64>,
    pub(super) call_id: Option<String>,
    pub(super) saw_arguments: bool,
}

pub(super) fn process_text_delta(
    state: &mut OpenAiChatStreamState,
    // JSON: OpenAI Chat Completions streaming delta; upstream JSON is the contract.
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, WireStreamError>>,
) {
    let Some(text) = delta.get("content").and_then(Value::as_str) else {
        return;
    };
    if text.is_empty() {
        return;
    }
    close_reasoning(state, events);
    let index = if let Some(index) = state.text_block {
        index
    } else {
        let index = state.next_index;
        state.next_index += 1;
        state.text_block = Some(index);
        events.push(Ok(CanonicalEvent::ContentBlockStart {
            index,
            block: ContentBlockKind::Text,
        }));
        index
    };
    events.push(Ok(CanonicalEvent::TextDelta {
        index,
        text: text.to_owned(),
    }));
}

pub(super) fn close_reasoning(
    state: &mut OpenAiChatStreamState,
    events: &mut Vec<Result<CanonicalEvent, WireStreamError>>,
) {
    if let Some(index) = state.reasoning_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
}

// Why: DeepSeek, Qwen and Moonshot stream reasoning in the nonstandard
// `reasoning_content` field; some compatible providers spell it `reasoning`.
pub(super) fn process_reasoning_delta(
    state: &mut OpenAiChatStreamState,
    // JSON: OpenAI Chat Completions streaming delta; upstream JSON is the contract.
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, WireStreamError>>,
) {
    let Some(text) = delta
        .get("reasoning_content")
        .or_else(|| delta.get("reasoning"))
        .and_then(Value::as_str)
    else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let index = if let Some(index) = state.reasoning_block {
        index
    } else {
        let index = state.next_index;
        state.next_index += 1;
        state.reasoning_block = Some(index);
        events.push(Ok(CanonicalEvent::ContentBlockStart {
            index,
            block: ContentBlockKind::Thinking {
                id: None,
                signature: None,
            },
        }));
        index
    };
    events.push(Ok(CanonicalEvent::ThinkingDelta {
        index,
        text: text.to_owned(),
    }));
}

pub(super) fn process_tool_calls(
    state: &mut OpenAiChatStreamState,
    // JSON: OpenAI Chat Completions streaming delta; upstream JSON is the contract.
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, WireStreamError>>,
) {
    let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) else {
        return;
    };
    for tc in tool_calls {
        let slot =
            existing_tool_call(state, tc).unwrap_or_else(|| open_new_tool_call(state, tc, events));
        if let Some(args) = tc
            .get("function")
            .and_then(|f| f.get("arguments"))
            .and_then(Value::as_str)
            && !args.is_empty()
        {
            state.tool_calls[slot].saw_arguments = true;
            events.push(Ok(CanonicalEvent::ToolUseDelta {
                index: state.tool_calls[slot].index,
                partial_json: args.to_owned(),
            }));
        }
    }
}

// Why: some OpenAI-compatible upstreams omit `index` on tool-call deltas; a
// fresh `id` then marks a new call and a delta with neither continues the
// latest one.
fn existing_tool_call(
    state: &OpenAiChatStreamState,
    // JSON: OpenAI Chat Completions streaming tool-call delta.
    tc: &Value,
) -> Option<usize> {
    if let Some(provider_index) = tc.get("index").and_then(Value::as_i64) {
        return state
            .tool_calls
            .iter()
            .position(|p| p.provider_index == Some(provider_index));
    }
    call_id(tc).map_or_else(
        || state.tool_calls.len().checked_sub(1),
        |id| {
            state
                .tool_calls
                .iter()
                .position(|p| p.call_id.as_deref() == Some(id))
        },
    )
}

// JSON: OpenAI Chat Completions streaming tool-call delta.
fn call_id(tc: &Value) -> Option<&str> {
    tc.get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
}

fn open_new_tool_call(
    state: &mut OpenAiChatStreamState,
    // JSON: OpenAI Chat Completions streaming tool-call delta.
    tc: &Value,
    events: &mut Vec<Result<CanonicalEvent, WireStreamError>>,
) -> usize {
    close_reasoning(state, events);
    state.saw_tool_call = true;
    let idx = state.next_index;
    state.next_index += 1;
    let id = call_id(tc).map(str::to_owned);
    let name = tc
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    events.push(Ok(CanonicalEvent::ContentBlockStart {
        index: idx,
        block: ContentBlockKind::ToolUse {
            id: id.clone().unwrap_or_default(),
            name,
            signature: None,
        },
    }));
    state.tool_calls.push(ToolCallProgress {
        index: idx,
        provider_index: tc.get("index").and_then(Value::as_i64),
        call_id: id,
        saw_arguments: false,
    });
    state.tool_calls.len() - 1
}
