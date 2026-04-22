use serde_json::Value;

use super::audit::{CapturedToolUse, CapturedUsage};

pub fn extract_from_anthropic_response(bytes: &[u8]) -> (CapturedUsage, Vec<CapturedToolUse>) {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return (CapturedUsage::default(), Vec::new());
    };
    extract_from_anthropic_value(&value)
}

pub fn extract_assistant_text(bytes: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<Value>(bytes).ok()?;
    let content = value.get("content")?.as_array()?;
    let mut out = String::new();
    for block in content {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

pub fn extract_from_anthropic_value(value: &Value) -> (CapturedUsage, Vec<CapturedToolUse>) {
    let usage = CapturedUsage {
        input_tokens: value
            .get("usage")
            .and_then(|u| u.get("input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        output_tokens: value
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
    };

    let mut tool_calls = Vec::new();
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        for block in content {
            if block.get("type").and_then(Value::as_str) == Some("tool_use") {
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
                let input = block
                    .get("input")
                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                    .unwrap_or_default();
                tool_calls.push(CapturedToolUse {
                    ai_tool_call_id: id,
                    tool_name: name,
                    tool_input: input,
                });
            }
        }
    }

    (usage, tool_calls)
}
