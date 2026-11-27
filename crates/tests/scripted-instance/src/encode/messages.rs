//! A turn as an Anthropic Messages response: the server-sent event stream
//! and the buffered body.

use serde_json::{Value, json};

use super::{Call, argument_chunks, stop_of, usage_of};
use crate::script::{Respond, StopReason, Turn};

pub(crate) const TERMINAL_FRAMES: usize = 2;

fn frame(event: &Value) -> String {
    let name = event.get("type").and_then(Value::as_str).unwrap_or("ping");
    format!("event: {name}\ndata: {event}\n\n")
}

fn message_id(call: Call<'_>) -> String {
    format!("msg_scripted_{}", call.sequence)
}

const fn stop_reason(stop: StopReason) -> &'static str {
    match stop {
        StopReason::End => "end_turn",
        StopReason::ToolUse => "tool_use",
    }
}

pub(crate) fn frames(turn: &Turn, call: Call<'_>) -> Vec<String> {
    let usage = usage_of(turn);
    let mut events = vec![json!({
        "type": "message_start",
        "message": {
            "id": message_id(call),
            "type": "message",
            "role": "assistant",
            "model": call.model,
            "content": [],
            "stop_reason": null,
            "stop_sequence": null,
            "usage": { "input_tokens": usage.input, "output_tokens": 0 },
        },
    })];
    for (index, part) in turn.respond.iter().enumerate() {
        match part {
            Respond::Text(text) => {
                events.push(json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": { "type": "text", "text": "" },
                }));
                events.push(json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": { "type": "text_delta", "text": text },
                }));
            },
            Respond::ToolCall(tool) => {
                events.push(json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": {
                        "type": "tool_use",
                        "id": tool.id,
                        "name": tool.name,
                        "input": {},
                    },
                }));
                events.extend(argument_chunks(tool).into_iter().map(|chunk| {
                    json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": { "type": "input_json_delta", "partial_json": chunk },
                    })
                }));
            },
        }
        events.push(json!({ "type": "content_block_stop", "index": index }));
    }
    events.push(json!({
        "type": "message_delta",
        "delta": { "stop_reason": stop_reason(stop_of(turn)), "stop_sequence": null },
        "usage": { "output_tokens": usage.output },
    }));
    events.push(json!({ "type": "message_stop" }));
    events.iter().map(frame).collect()
}

pub(crate) fn body(turn: &Turn, call: Call<'_>) -> Value {
    let usage = usage_of(turn);
    let content: Vec<Value> = turn
        .respond
        .iter()
        .map(|part| match part {
            Respond::Text(text) => json!({ "type": "text", "text": text }),
            Respond::ToolCall(tool) => json!({
                "type": "tool_use",
                "id": tool.id,
                "name": tool.name,
                "input": tool.input,
            }),
        })
        .collect();
    json!({
        "id": message_id(call),
        "type": "message",
        "role": "assistant",
        "model": call.model,
        "content": content,
        "stop_reason": stop_reason(stop_of(turn)),
        "stop_sequence": null,
        "usage": { "input_tokens": usage.input, "output_tokens": usage.output },
    })
}
