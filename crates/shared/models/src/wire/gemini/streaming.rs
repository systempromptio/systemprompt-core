//! Maps the Gemini `?alt=sse` byte stream to canonical events.
//!
//! Each SSE `data:` frame carries a full [`GeminiResponse`] chunk whose
//! candidate parts are incremental. Thought parts (`"thought": true`) and
//! answer text each stream as deltas on their own content block;
//! `functionCall` parts emit a complete tool-use block (Gemini sends each call
//! whole rather than as partial JSON).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
use serde_json::Value;
use uuid::Uuid;

use super::response::stop_reason;
use super::wire::{GeminiPart, GeminiResponse};
use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, CanonicalUsageUpdate, ContentBlockKind,
};

struct StreamState {
    buf: Vec<u8>,
    model: String,
    message_id: String,
    started: bool,
    text_block: Option<u32>,
    thinking_block: Option<u32>,
    next_index: u32,
    // Why: Gemini reports finishReason STOP even on a turn whose candidate is a
    // functionCall, so the wire's own reason cannot distinguish "finished
    // talking" from "wants a tool run". Tracking it here is the only signal.
    emitted_tool_use: bool,
}

pub fn sse_to_canonical_events<S, E>(
    stream: S,
    fallback_model: String,
) -> BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::fmt::Display + 'static,
{
    let initial = StreamState {
        buf: Vec::new(),
        emitted_tool_use: false,
        model: fallback_model,
        message_id: format!("msg_{}", Uuid::new_v4().simple()),
        started: false,
        text_block: None,
        thinking_block: None,
        next_index: 0,
    };

    stream
        .map(|chunk| chunk.map_err(|e| e.to_string()))
        .scan(initial, |state, item| {
            let res = match item {
                Ok(bytes) => drain_buffer(state, &bytes),
                Err(e) => vec![Err(e)],
            };
            futures_util::future::ready(Some(res))
        })
        .flat_map(stream::iter)
        .boxed()
}

fn drain_buffer(state: &mut StreamState, bytes: &[u8]) -> Vec<Result<CanonicalEvent, String>> {
    state.buf.extend_from_slice(bytes);
    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
    while let Some(end) = crate::wire::sse::frame_end(&state.buf) {
        let frame: Vec<u8> = state.buf.drain(..end).collect();
        let frame_str = String::from_utf8_lossy(&frame);
        for line in frame_str.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };
            handle_chunk(state, &value, &mut events);
        }
    }
    events
}

fn handle_chunk(
    state: &mut StreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let Ok(chunk) = serde_json::from_value::<GeminiResponse>(value.clone()) else {
        return;
    };
    if !state.started {
        emit_start(state, &chunk, events);
    }
    if let Some(usage) = chunk.usage_metadata {
        // Why: cachedContentTokenCount is a subset of promptTokenCount, and
        // `CanonicalUsage::input_tokens` is exclusive of cache reads. The cached
        // count must also be carried: omitted, a streamed reply reports zero
        // cache where the buffered parse of the same reply reports it, so the
        // two paths bill differently.
        events.push(Ok(CanonicalEvent::UsageDelta(CanonicalUsageUpdate {
            input_tokens: Some(usage.prompt.saturating_sub(usage.cached)),
            output_tokens: Some(usage.candidates + usage.thoughts),
            cache_read_tokens: Some(usage.cached),
            reasoning_tokens: Some(usage.thoughts),
            total_tokens: (usage.total > 0).then_some(usage.total),
            ..CanonicalUsageUpdate::default()
        })));
    }
    let Some(candidate) = chunk.candidates.into_iter().next() else {
        return;
    };
    if let Some(content) = candidate.content {
        for part in &content.parts {
            emit_part(state, part, events);
        }
    }
    if let Some(finish) = candidate.finish_reason.as_deref() {
        // Why: a turn that emitted a functionCall is a tool-use turn whatever
        // Gemini calls it. Reporting EndTurn here renders as
        // `finish_reason: "stop"` on the OpenAI surface, and a client that
        // follows that contract treats the turn as complete and never runs the
        // tool -- the call is present in the payload and silently ignored.
        // MAX_TOKENS is not overridden: a call cut mid-turn is not runnable.
        let reason = stop_reason(finish).with_tool_use(state.emitted_tool_use);
        emit_stop(state, reason, events);
    }
}

fn emit_stop(
    state: &mut StreamState,
    reason: CanonicalStopReason,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    close_thinking(state, events);
    close_text(state, events);
    events.push(Ok(CanonicalEvent::MessageStop {
        id: state.message_id.clone(),
        stop_reason: Some(reason),
    }));
}

fn close_text(state: &mut StreamState, events: &mut Vec<Result<CanonicalEvent, String>>) {
    if let Some(index) = state.text_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
}

fn close_thinking(state: &mut StreamState, events: &mut Vec<Result<CanonicalEvent, String>>) {
    if let Some(index) = state.thinking_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
}

fn emit_start(
    state: &mut StreamState,
    chunk: &GeminiResponse,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if let Some(id) = &chunk.response_id {
        state.message_id.clone_from(id);
    }
    if let Some(model) = &chunk.model_version {
        state.model.clone_from(model);
    }
    events.push(Ok(CanonicalEvent::MessageStart {
        id: state.message_id.clone(),
        model: state.model.clone(),
        usage: CanonicalUsage::default(),
    }));
    state.started = true;
}

fn emit_part(
    state: &mut StreamState,
    part: &GeminiPart,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    match part {
        GeminiPart::Text {
            text,
            thought: Some(true),
            thought_signature,
        } => emit_thought(state, text, thought_signature.clone(), events),
        GeminiPart::Text { text, .. } if !text.is_empty() => emit_text(state, text, events),
        GeminiPart::FunctionCall {
            function_call,
            thought_signature,
        } => {
            close_thinking(state, events);
            state.emitted_tool_use = true;
            emit_tool_use(
                state,
                &function_call.name,
                &function_call.args,
                thought_signature.clone(),
                events,
            );
        },
        _ => {},
    }
}

fn emit_text(
    state: &mut StreamState,
    text: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    close_thinking(state, events);
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

fn emit_thought(
    state: &mut StreamState,
    text: &str,
    signature: Option<String>,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let index = if let Some(index) = state.thinking_block {
        index
    } else {
        let index = state.next_index;
        state.next_index += 1;
        state.thinking_block = Some(index);
        events.push(Ok(CanonicalEvent::ContentBlockStart {
            index,
            block: ContentBlockKind::Thinking {
                id: None,
                signature: None,
            },
        }));
        index
    };
    if !text.is_empty() {
        events.push(Ok(CanonicalEvent::ThinkingDelta {
            index,
            text: text.to_owned(),
        }));
    }
    if let Some(signature) = signature {
        events.push(Ok(CanonicalEvent::SignatureDelta { index, signature }));
    }
}

fn emit_tool_use(
    state: &mut StreamState,
    name: &str,
    args: &Value,
    signature: Option<String>,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let index = state.next_index;
    state.next_index += 1;
    events.push(Ok(CanonicalEvent::ContentBlockStart {
        index,
        block: ContentBlockKind::ToolUse {
            id: format!("call_{}", Uuid::new_v4().simple()),
            name: name.to_owned(),
            signature,
        },
    }));
    events.push(Ok(CanonicalEvent::ToolUseDelta {
        index,
        partial_json: args.to_string(),
    }));
    events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
}
