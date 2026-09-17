//! Maps the Gemini `?alt=sse` byte stream to canonical events.
//!
//! Each SSE `data:` frame carries a full [`GeminiResponse`] chunk whose
//! candidate parts are incremental. Thought parts (`"thought": true`) and
//! answer text each stream as deltas on their own content block;
//! `functionCall` parts emit a complete tool-use block (Gemini sends each call
//! whole rather than as partial JSON).
//!
//! A candidate that finishes on a reason other than `STOP`/`MAX_TOKENS`
//! without having emitted a part is an upstream error, not an empty turn, and
//! a plain JSON `{"error": …}` body left unterminated at end of stream is
//! drained and surfaced the same way — Vertex answers a rejected replay with
//! exactly that shape and no SSE framing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
use serde_json::Value;
use uuid::Uuid;

use super::response::{blocked_prompt_message, empty_terminal_message, stop_reason};
use super::streaming_parts::{close_text, close_thinking, emit_part};
use super::wire::GeminiResponse;
use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, CanonicalUsageUpdate,
};

pub(super) struct StreamState {
    pub(super) buf: Vec<u8>,
    pub(super) model: String,
    pub(super) message_id: String,
    pub(super) started: bool,
    pub(super) text_block: Option<u32>,
    pub(super) thinking_block: Option<u32>,
    pub(super) next_index: u32,
    pub(super) emitted_tool_use: bool,
    // Why: thoughts do not count — a turn that only thought and then finished
    // on MALFORMED_FUNCTION_CALL is still an empty answer to the client.
    pub(super) emitted_part: bool,
    pub(super) stopped: bool,
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
        emitted_part: false,
        stopped: false,
        model: fallback_model,
        message_id: format!("msg_{}", Uuid::new_v4().simple()),
        started: false,
        text_block: None,
        thinking_block: None,
        next_index: 0,
    };

    stream
        .map(|chunk| Some(chunk.map_err(|e| e.to_string())))
        .chain(stream::once(futures_util::future::ready(None)))
        .scan(initial, |state, item| {
            let res = match item {
                Some(Ok(bytes)) => drain_buffer(state, &bytes),
                Some(Err(e)) => vec![Err(e)],
                None => drain_tail(state),
            };
            futures_util::future::ready(Some(res))
        })
        .flat_map(stream::iter)
        .boxed()
}

// Why: a body that never carried a frame terminator is not an SSE stream at
// all; when it is a JSON error object it must surface as the upstream error
// it is rather than finalising downstream as "empty upstream stream".
fn drain_tail(state: &mut StreamState) -> Vec<Result<CanonicalEvent, String>> {
    if state.stopped {
        return Vec::new();
    }
    let tail = String::from_utf8_lossy(&state.buf);
    let tail = tail.trim();
    if tail.is_empty() {
        return Vec::new();
    }
    let body = tail.strip_prefix("data:").map_or(tail, str::trim);
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    if let Some(message) = crate::wire::sse::upstream_error_message(&value) {
        events.push(Ok(CanonicalEvent::Error(message)));
    } else {
        handle_chunk(state, &value, &mut events);
    }
    state.buf.clear();
    events
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
            match serde_json::from_str::<Value>(data.trim()) {
                Ok(value) => handle_chunk(state, &value, &mut events),
                Err(e) => events.push(Err(format!("malformed Gemini SSE frame: {e}"))),
            }
        }
    }
    events
}

fn handle_chunk(
    state: &mut StreamState,
    // JSON: Gemini streaming frame; upstream JSON is the contract.
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if let Some(message) = crate::wire::sse::upstream_error_message(value) {
        events.push(Ok(CanonicalEvent::Error(message)));
        return;
    }
    let Ok(chunk) = serde_json::from_value::<GeminiResponse>(value.clone()) else {
        return;
    };
    // Why: Gemini can report a blocked prompt with only `promptFeedback`,
    // without candidates or a finish reason.
    if let Some(reason) = chunk
        .prompt_feedback
        .as_ref()
        .and_then(|f| f.block_reason.as_deref())
    {
        let detail = chunk
            .prompt_feedback
            .as_ref()
            .and_then(|f| f.block_reason_message.as_deref());
        events.push(Ok(CanonicalEvent::Error(blocked_prompt_message(
            reason, detail,
        ))));
        return;
    }
    if !state.started {
        emit_start(state, &chunk, events);
    }
    if let Some(usage) = chunk.usage_metadata {
        // Why: Gemini includes `cachedContentTokenCount` in `promptTokenCount`.
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
        // Why: Gemini reports `finishReason: STOP` even for a `functionCall` candidate.
        let reason = stop_reason(finish).with_tool_use(state.emitted_tool_use);
        if reason.empty_terminal_is_error() && !state.emitted_part {
            state.stopped = true;
            events.push(Ok(CanonicalEvent::Error(empty_terminal_message(
                finish,
                candidate.finish_message.as_deref(),
            ))));
            return;
        }
        emit_stop(state, reason, finish, events);
    }
}

fn emit_stop(
    state: &mut StreamState,
    reason: CanonicalStopReason,
    finish: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    close_thinking(state, events);
    close_text(state, events);
    state.stopped = true;
    events.push(Ok(CanonicalEvent::MessageStop {
        id: state.message_id.clone(),
        stop_reason: Some(reason),
        raw_finish_reason: Some(finish.to_owned()),
    }));
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
