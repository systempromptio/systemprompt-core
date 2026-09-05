//! `OpenAI` Chat Completions SSE-to-[`CanonicalEvent`] translation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use core::fmt::Display;

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::Value;
use systemprompt_identifiers::MessageId;

use super::stream_delta::{
    OpenAiChatStreamState, close_reasoning, process_reasoning_delta, process_text_delta,
    process_tool_calls,
};
use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, CanonicalUsageUpdate,
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
        text_block: None,
        next_index: 0,
        tool_calls: Vec::new(),
        reasoning_block: None,
        saw_tool_call: false,
        stopped: false,
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
                if !state.stopped {
                    emit_message_stop(state, "stop", &mut events);
                }
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
    process_reasoning_delta(state, delta, events);
    process_text_delta(state, delta, events);
    process_tool_calls(state, delta, events);
    if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str)
        && !state.stopped
    {
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

fn emit_message_stop(
    state: &mut OpenAiChatStreamState,
    finish: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    state.stopped = true;
    close_reasoning(state, events);
    if let Some(index) = state.text_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
    for tc in state.tool_calls.drain(..) {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index: tc.index }));
    }
    events.push(Ok(CanonicalEvent::MessageStop {
        id: state.message_id.as_str().to_owned(),
        stop_reason: Some(
            CanonicalStopReason::from_openai(finish).with_tool_use(state.saw_tool_call),
        ),
    }));
}

fn usage_from_value(usage: &Value) -> CanonicalUsageUpdate {
    let field = |name: &str| usage.get(name).and_then(Value::as_u64).map(|v| v as u32);
    CanonicalUsageUpdate {
        input_tokens: field("prompt_tokens"),
        output_tokens: field("completion_tokens"),
        cache_read_tokens: usage
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        cache_creation_tokens: None,
        // Why: already inside `completion_tokens` on this contract, so it is
        // reported as a breakdown and never added to the total.
        reasoning_tokens: usage
            .get("completion_tokens_details")
            .and_then(|d| d.get("reasoning_tokens"))
            .and_then(Value::as_u64)
            .map(|v| v as u32),
    }
}
