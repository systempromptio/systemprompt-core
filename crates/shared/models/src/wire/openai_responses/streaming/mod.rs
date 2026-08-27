//! `OpenAI` Responses SSE pipeline: upstream byte stream → canonical events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod delta;

use core::fmt::Display;

use futures_util::stream::BoxStream;
use futures_util::{Stream, StreamExt};
// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::Value;

use delta::{DeltaShape, emit_delta, handle_completed, handle_error, handle_item_done};

use super::slot::{ItemSlot, ResponsesStreamState, SlotKind, SlotKindMatch};
use crate::wire::canonical::{CanonicalEvent, CanonicalUsage, ContentBlockKind};

pub fn sse_to_canonical_events<S, E>(
    stream: S,
    fallback_model: String,
) -> BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: Display + 'static,
{
    use futures_util::stream;
    let initial = ResponsesStreamState {
        buf: Vec::new(),
        model: fallback_model,
        response_id: String::new(),
        started: false,
        items: Vec::new(),
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
    state: &mut ResponsesStreamState,
    bytes: &bytes::Bytes,
) -> Vec<Result<CanonicalEvent, String>> {
    state.buf.extend_from_slice(bytes);
    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
    while let Some(end) = crate::wire::sse::frame_end(&state.buf) {
        let frame: Vec<u8> = state.buf.drain(..end).collect();
        let frame_str = String::from_utf8_lossy(&frame);
        let mut data_parts: Vec<&str> = Vec::new();
        for line in frame_str.lines() {
            if let Some(d) = line.strip_prefix("data: ") {
                data_parts.push(d);
            }
        }
        let joined = data_parts.join("\n");
        if joined.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&joined) {
            handle_responses_event(state, &value, &mut events);
        }
    }
    events
}

fn handle_responses_event(
    state: &mut ResponsesStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return;
    };
    match kind {
        "response.created" => handle_created(state, value, events),
        "response.output_item.added" => handle_item_added(state, value, events),
        "response.output_text.delta" => {
            emit_delta(
                state,
                value,
                SlotKindMatch::Message,
                events,
                DeltaShape::Text,
            );
        },
        "response.function_call_arguments.delta" => {
            emit_delta(
                state,
                value,
                SlotKindMatch::Function,
                events,
                DeltaShape::ToolUse,
            );
        },
        "response.reasoning_summary_text.delta" => {
            emit_delta(
                state,
                value,
                SlotKindMatch::Reasoning,
                events,
                DeltaShape::Thinking,
            );
        },
        "response.output_item.done" => handle_item_done(state, value, events),
        "response.completed" => handle_completed(state, value, events, false),
        "response.incomplete" => handle_completed(state, value, events, true),
        "response.failed" | "error" => handle_error(value, events),
        _ => {},
    }
}

fn handle_created(
    state: &mut ResponsesStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let response = value.get("response").unwrap_or(&Value::Null);
    let id = response
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("resp_unknown")
        .to_owned();
    let model = response
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&state.model)
        .to_owned();
    state.model.clone_from(&model);
    state.response_id.clone_from(&id);
    state.started = true;
    events.push(Ok(CanonicalEvent::MessageStart {
        id,
        model,
        usage: CanonicalUsage::default(),
    }));
}

fn handle_item_added(
    state: &mut ResponsesStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let output_index = value
        .get("output_index")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let item = value.get("item").unwrap_or(&Value::Null);
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
    let canonical_index = state.items.len() as u32;
    let (kind, block) = match item_type {
        "message" => (SlotKind::Message, ContentBlockKind::Text),
        "function_call" => {
            let id = item
                .get("call_id")
                .and_then(Value::as_str)
                .or_else(|| item.get("id").and_then(Value::as_str))
                .unwrap_or("")
                .to_owned();
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            (
                SlotKind::Function,
                ContentBlockKind::ToolUse {
                    id,
                    name,
                    signature: None,
                },
            )
        },
        "reasoning" => (
            SlotKind::Reasoning,
            ContentBlockKind::Thinking {
                id: item.get("id").and_then(Value::as_str).map(str::to_owned),
                signature: None,
            },
        ),
        _ => return,
    };
    state.items.push(ItemSlot {
        output_index,
        canonical_index,
        kind,
    });
    events.push(Ok(CanonicalEvent::ContentBlockStart {
        index: canonical_index,
        block,
    }));
}
