//! `OpenAI` Chat Completions SSE-to-[`CanonicalEvent`] translation.

use core::fmt::Display;

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::Value;
use systemprompt_identifiers::MessageId;

use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

pub fn sse_to_canonical_events<S, E>(
    stream: S,
    fallback_model: String,
) -> BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Display,
{
    let initial = OpenAiChatStreamState {
        buf: Vec::new(),
        model: fallback_model,
        message_id: MessageId::new(""),
        started: false,
        text_block_open: false,
        next_index: 0,
        tool_calls: Vec::new(),
    };

    let s = stream
        .map(|chunk| chunk.map_err(|e| e.to_string()))
        .scan(initial, |state, item| {
            let res = match item {
                Ok(bytes) => Some(drain_buffer(state, &bytes)),
                Err(e) => Some(vec![Err(e)]),
            };
            futures_util::future::ready(res)
        })
        .flat_map(stream::iter);
    s.boxed()
}

fn drain_buffer(
    state: &mut OpenAiChatStreamState,
    bytes: &Bytes,
) -> Vec<Result<CanonicalEvent, String>> {
    state.buf.extend_from_slice(bytes);
    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
    while let Some(end) = crate::wire::sse::frame_end(&state.buf) {
        let frame: Vec<u8> = state.buf.drain(..end).collect();
        let frame_str = String::from_utf8_lossy(&frame);
        for line in frame_str.lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data.trim() == "[DONE]" {
                if state.text_block_open {
                    events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
                    state.text_block_open = false;
                }
                events.push(Ok(CanonicalEvent::MessageStop {
                    id: state.message_id.as_str().to_owned(),
                    stop_reason: Some(CanonicalStopReason::EndTurn),
                }));
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            handle_chunk(state, &value, &mut events);
        }
    }
    events
}

fn handle_chunk(
    state: &mut OpenAiChatStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if !state.started {
        emit_message_start(state, value, events);
    }
    if let Some(usage) = value.get("usage") {
        events.push(Ok(CanonicalEvent::UsageDelta(usage_from_value(usage))));
    }
    let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    else {
        return;
    };
    let delta = choice.get("delta").unwrap_or(&Value::Null);
    process_text_delta(state, delta, events);
    process_tool_calls(state, delta, events);
    if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str) {
        emit_message_stop(state, finish, events);
    }
}

fn emit_message_start(
    state: &mut OpenAiChatStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("msg_openai")
        .to_owned();
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&state.model)
        .to_owned();
    state.message_id = MessageId::new(&id);
    events.push(Ok(CanonicalEvent::MessageStart {
        id,
        model: model.clone(),
        usage: CanonicalUsage::default(),
    }));
    state.model = model;
    state.started = true;
}

fn process_text_delta(
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

fn process_tool_calls(
    state: &mut OpenAiChatStreamState,
    delta: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) else {
        return;
    };
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

fn open_new_tool_call(
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

fn emit_message_stop(
    state: &mut OpenAiChatStreamState,
    finish: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if state.text_block_open {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
        state.text_block_open = false;
    }
    for tc in state.tool_calls.drain(..) {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index: tc.index }));
    }
    events.push(Ok(CanonicalEvent::MessageStop {
        id: state.message_id.as_str().to_owned(),
        stop_reason: Some(CanonicalStopReason::from_openai(finish)),
    }));
}

fn usage_from_value(usage: &Value) -> CanonicalUsage {
    let field = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0) as u32;
    CanonicalUsage {
        input_tokens: field("prompt_tokens"),
        output_tokens: field("completion_tokens"),
        cache_read_tokens: usage
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        cache_creation_tokens: 0,
        total_tokens: field("total_tokens"),
    }
}

struct OpenAiChatStreamState {
    buf: Vec<u8>,
    model: String,
    message_id: MessageId,
    started: bool,
    text_block_open: bool,
    next_index: u32,
    tool_calls: Vec<ToolCallProgress>,
}

struct ToolCallProgress {
    index: u32,
    provider_index: i64,
}
