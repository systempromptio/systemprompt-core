//! Maps the Gemini `?alt=sse` byte stream to canonical events.
//!
//! Each SSE `data:` frame carries a full [`GeminiResponse`] chunk whose
//! candidate parts are incremental. Text parts stream as deltas on block 0;
//! `functionCall` parts emit a complete tool-use block (Gemini sends each call
//! whole rather than as partial JSON).

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
use serde_json::Value;
use uuid::Uuid;

use super::response::stop_reason;
use super::wire::{GeminiPart, GeminiResponse};
use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

struct StreamState {
    buf: Vec<u8>,
    model: String,
    message_id: String,
    started: bool,
    text_block_open: bool,
    next_index: u32,
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
        model: fallback_model,
        message_id: format!("msg_{}", Uuid::new_v4().simple()),
        started: false,
        text_block_open: false,
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
    while let Some(pos) = find_double_newline(&state.buf) {
        let frame: Vec<u8> = state.buf.drain(..pos + 2).collect();
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

fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
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
        events.push(Ok(CanonicalEvent::UsageDelta(CanonicalUsage {
            input_tokens: usage.prompt,
            output_tokens: usage.candidates,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            total_tokens: usage.prompt + usage.candidates,
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
        emit_stop(state, stop_reason(finish), events);
    }
}

fn emit_stop(
    state: &mut StreamState,
    reason: CanonicalStopReason,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if state.text_block_open {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
        state.text_block_open = false;
    }
    events.push(Ok(CanonicalEvent::MessageStop {
        id: state.message_id.clone(),
        stop_reason: Some(reason),
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

fn emit_part(
    state: &mut StreamState,
    part: &GeminiPart,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    match part {
        GeminiPart::Text { text } if !text.is_empty() => emit_text(state, text, events),
        GeminiPart::FunctionCall { function_call } => {
            emit_tool_use(state, &function_call.name, &function_call.args, events);
        },
        _ => {},
    }
}

fn emit_text(
    state: &mut StreamState,
    text: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if !state.text_block_open {
        events.push(Ok(CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        }));
        state.text_block_open = true;
        if state.next_index == 0 {
            state.next_index = 1;
        }
    }
    events.push(Ok(CanonicalEvent::TextDelta {
        index: 0,
        text: text.to_owned(),
    }));
}

fn emit_tool_use(
    state: &mut StreamState,
    name: &str,
    args: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let index = state.next_index;
    state.next_index += 1;
    events.push(Ok(CanonicalEvent::ContentBlockStart {
        index,
        block: ContentBlockKind::ToolUse {
            id: format!("call_{}", Uuid::new_v4().simple()),
            name: name.to_owned(),
        },
    }));
    events.push(Ok(CanonicalEvent::ToolUseDelta {
        index,
        partial_json: args.to_string(),
    }));
    events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
}
