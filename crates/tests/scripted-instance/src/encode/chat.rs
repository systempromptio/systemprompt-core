//! A turn as an OpenAI Chat Completions response: the chunk stream and the
//! buffered body.

use serde_json::{Value, json};

use super::{Call, argument_chunks, stop_of, text_of, tool_calls, usage_of};
use crate::script::{Respond, StopReason, Turn};

const DONE_FRAME: &str = "data: [DONE]\n\n";

pub(crate) const TERMINAL_FRAMES: usize = 3;

fn completion_id(call: Call<'_>) -> String {
    format!("chatcmpl-scripted-{}", call.sequence)
}

const fn finish_reason(stop: StopReason) -> &'static str {
    match stop {
        StopReason::End => "stop",
        StopReason::ToolUse => "tool_calls",
        StopReason::MaxTokens => "length",
        StopReason::ContentFilter => "content_filter",
    }
}

fn usage(turn: &Turn) -> Value {
    let usage = usage_of(turn);
    json!({
        "prompt_tokens": usage.input,
        "completion_tokens": usage.output,
        "total_tokens": usage.input.saturating_add(usage.output),
    })
}

fn chunk(call: Call<'_>, choices: Value) -> Value {
    json!({
        "id": completion_id(call),
        "object": "chat.completion.chunk",
        "created": 0,
        "model": call.model,
        "choices": choices,
    })
}

fn delta(call: Call<'_>, delta: Value, finish: Option<&str>) -> Value {
    chunk(
        call,
        json!([{ "index": 0, "delta": delta, "finish_reason": finish }]),
    )
}

pub(crate) fn frames(turn: &Turn, call: Call<'_>) -> Vec<String> {
    let mut chunks = vec![delta(call, json!({ "role": "assistant" }), None)];
    let mut tool_index = 0_u32;
    for part in &turn.respond {
        match part {
            Respond::Text(text) => chunks.push(delta(call, json!({ "content": text }), None)),
            Respond::ToolCall(tool) => {
                chunks.push(delta(
                    call,
                    json!({ "tool_calls": [{
                        "index": tool_index,
                        "id": tool.id,
                        "type": "function",
                        "function": { "name": tool.name, "arguments": "" },
                    }] }),
                    None,
                ));
                chunks.extend(argument_chunks(tool).into_iter().map(|arguments| {
                    delta(
                        call,
                        json!({ "tool_calls": [{
                            "index": tool_index,
                            "function": { "arguments": arguments },
                        }] }),
                        None,
                    )
                }));
                tool_index += 1;
            },
        }
    }
    chunks.push(delta(call, json!({}), Some(finish_reason(stop_of(turn)))));
    let mut totals = chunk(call, json!([]));
    totals["usage"] = usage(turn);
    chunks.push(totals);
    chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .chain(std::iter::once(DONE_FRAME.to_owned()))
        .collect()
}

pub(crate) fn body(turn: &Turn, call: Call<'_>) -> Value {
    let calls: Vec<Value> = tool_calls(turn)
        .map(|tool| {
            json!({
                "id": tool.id,
                "type": "function",
                "function": { "name": tool.name, "arguments": tool.input.to_string() },
            })
        })
        .collect();
    let text = text_of(turn);
    let mut message = json!({
        "role": "assistant",
        "content": if text.is_empty() { Value::Null } else { Value::String(text) },
    });
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    json!({
        "id": completion_id(call),
        "object": "chat.completion",
        "created": 0,
        "model": call.model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason(stop_of(turn)),
        }],
        "usage": usage(turn),
    })
}
