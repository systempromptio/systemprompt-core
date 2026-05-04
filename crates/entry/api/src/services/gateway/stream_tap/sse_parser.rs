use serde_json::Value;

use super::super::captures::CapturedToolUse;
use super::{PartialToolUse, TapState};

pub(super) fn drain_sse(state: &mut TapState) {
    loop {
        let Some(pos) = find_double_newline(&state.sse_buffer) else {
            return;
        };
        let frame_bytes: Vec<u8> = state.sse_buffer.drain(..pos + 2).collect();
        let frame = String::from_utf8_lossy(&frame_bytes);
        for line in frame.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                let Ok(json) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                handle_sse_event(state, &json);
            }
        }
    }
}

fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

fn handle_sse_event(state: &mut TapState, event: &Value) {
    let Some(kind) = event.get("type").and_then(Value::as_str) else {
        return;
    };
    match kind {
        "message_start" => handle_message_start(state, event),
        "message_delta" => handle_message_delta(state, event),
        "content_block_start" => handle_content_block_start(state, event),
        "content_block_delta" => handle_content_block_delta(state, event),
        "content_block_stop" => handle_content_block_stop(state, event),
        _ => {},
    }
}

fn handle_message_start(state: &mut TapState, event: &Value) {
    let Some(message) = event.get("message") else {
        return;
    };
    if let Some(model) = message.get("model").and_then(Value::as_str) {
        if !model.is_empty() {
            state.served_model = Some(model.to_string());
        }
    }
    if let Some(usage) = message.get("usage") {
        if let Some(v) = usage.get("input_tokens").and_then(Value::as_u64) {
            state.input_tokens = v as u32;
        }
        if let Some(v) = usage.get("output_tokens").and_then(Value::as_u64) {
            state.output_tokens = v as u32;
        }
    }
}

fn handle_message_delta(state: &mut TapState, event: &Value) {
    let Some(usage) = event.get("usage") else {
        return;
    };
    if let Some(v) = usage.get("output_tokens").and_then(Value::as_u64) {
        state.output_tokens = v as u32;
    }
    if let Some(v) = usage.get("input_tokens").and_then(Value::as_u64) {
        state.input_tokens = v as u32;
    }
}

fn handle_content_block_start(state: &mut TapState, event: &Value) {
    let index = event.get("index").and_then(Value::as_i64).unwrap_or(-1);
    let Some(block) = event.get("content_block") else {
        return;
    };
    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
        return;
    }
    let id = block
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let name = block
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    state.tool_uses_in_progress.push(PartialToolUse {
        index,
        id,
        name,
        input_json: String::new(),
    });
}

fn handle_content_block_delta(state: &mut TapState, event: &Value) {
    let index = event.get("index").and_then(Value::as_i64).unwrap_or(-1);
    let Some(delta) = event.get("delta") else {
        return;
    };
    if delta.get("type").and_then(Value::as_str) != Some("input_json_delta") {
        return;
    }
    let Some(partial) = delta.get("partial_json").and_then(Value::as_str) else {
        return;
    };
    if let Some(pt) = state
        .tool_uses_in_progress
        .iter_mut()
        .find(|p| p.index == index)
    {
        pt.input_json.push_str(partial);
    }
}

fn handle_content_block_stop(state: &mut TapState, event: &Value) {
    let index = event.get("index").and_then(Value::as_i64).unwrap_or(-1);
    if let Some(pos) = state
        .tool_uses_in_progress
        .iter()
        .position(|p| p.index == index)
    {
        let done = state.tool_uses_in_progress.remove(pos);
        state.tool_uses_done.push(CapturedToolUse {
            ai_tool_call_id: done.id,
            tool_name: done.name,
            tool_input: if done.input_json.is_empty() {
                "{}".to_string()
            } else {
                done.input_json
            },
        });
    }
}

pub(super) fn finalize_partials(state: &mut TapState) {
    let leftover: Vec<PartialToolUse> = std::mem::take(&mut state.tool_uses_in_progress);
    for p in leftover {
        state.tool_uses_done.push(CapturedToolUse {
            ai_tool_call_id: p.id,
            tool_name: p.name,
            tool_input: if p.input_json.is_empty() {
                "{}".to_string()
            } else {
                p.input_json
            },
        });
    }
}
