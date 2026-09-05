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

use crate::wire::canonical::{CanonicalEvent, ContentBlockKind};

pub(super) struct OpenAiChatStreamState {
    pub(super) buf: Vec<u8>,
    pub(super) model: String,
    pub(super) message_id: MessageId,
    pub(super) started: bool,
    pub(super) text_block: Option<u32>,
    pub(super) next_index: u32,
    pub(super) tool_calls: Vec<ToolCallProgress>,
    pub(super) reasoning_block: Option<u32>,
    // Why: the contract says a turn carrying tool_calls finishes with
    // "tool_calls", but several OpenAI-compatible upstreams send a plain
    // "stop" -- and a stream that ends on [DONE] alone states no reason at
    // all. Either renders as `finish_reason: "stop"` beside a complete
    // tool_calls array, and the client ends the turn without running it.
    pub(super) saw_tool_call: bool,
    // Why: a chunk carrying `finish_reason` and the `[DONE]` sentinel both end
    // the turn, and providers send both. Emitting MessageStop twice let the
    // sentinel's unconditional EndTurn land after a real `tool_calls` finish,
    // so the accumulated stop reason -- and the terminal frame every
    // OpenAI-contract client reads -- said "stop" on a turn that wanted a tool
    // run, and the tool call was silently dropped.
    pub(super) stopped: bool,
    // Why: the finish reason the wire stated, held until the stream ends so
    // the usage chunk that follows it lands before the canonical terminal.
    pub(super) pending_finish: Option<String>,
}

pub(super) struct ToolCallProgress {
    pub(super) index: u32,
    pub(super) provider_index: i64,
}

pub(super) fn process_text_delta(
    state: &mut OpenAiChatStreamState,
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
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

// Why: the reasoning track ends where the answer begins. Held open across the
// text deltas, the inbound Anthropic renderer keeps writing thinking_delta
// frames into a block the model has already left, and the client shows the
// answer as part of the model's private reasoning.
pub(super) fn close_reasoning(
    state: &mut OpenAiChatStreamState,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if let Some(index) = state.reasoning_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
}

// Why: the chat contract has no reasoning field, but every OpenAI-compatible
// provider that emits thinking (DeepSeek, Qwen, Moonshot) streams it here.
// The buffered parse already reads it; without the streaming half a thinking
// model's trace -- the whole point of those models -- is dropped mid-stream.
pub(super) fn process_reasoning_delta(
    state: &mut OpenAiChatStreamState,
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
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
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) else {
        return;
    };
    state.saw_tool_call = true;
    close_reasoning(state, events);
    for tc in tool_calls {
        let provider_index = tc.get("index").and_then(Value::as_i64).unwrap_or(-1);
        let existing = state
            .tool_calls
            .iter()
            .find(|p| p.provider_index == provider_index)
            .map(|p| p.index);
        let canonical_index =
            existing.unwrap_or_else(|| open_new_tool_call(state, tc, provider_index, events));
        if let Some(args) = tc
            .get("function")
            .and_then(|f| f.get("arguments"))
            .and_then(Value::as_str)
            && !args.is_empty()
        {
            events.push(Ok(CanonicalEvent::ToolUseDelta {
                index: canonical_index,
                partial_json: args.to_owned(),
            }));
        }
    }
}

pub(super) fn open_new_tool_call(
    state: &mut OpenAiChatStreamState,
    tc: &Value,
    provider_index: i64,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) -> u32 {
    let idx = state.next_index;
    state.next_index += 1;
    let id = tc
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let name = tc
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    events.push(Ok(CanonicalEvent::ContentBlockStart {
        index: idx,
        block: ContentBlockKind::ToolUse {
            id,
            name,
            signature: None,
        },
    }));
    state.tool_calls.push(ToolCallProgress {
        index: idx,
        provider_index,
    });
    idx
}
