// JSON: protocol boundary — OpenAI Responses outbound wire format is dynamic
// JSON.
use serde_json::{Map, Value};
use uuid::Uuid;

use super::super::super::canonical::CanonicalContent;
use super::super::super::canonical_response::{
    CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

pub(super) fn parse_response_object(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let id = value.get("id").and_then(Value::as_str).map_or_else(
        || format!("resp_{}", Uuid::new_v4().simple()),
        ToString::to_string,
    );
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_string();
    let usage = value
        .get("usage")
        .map(|u| CanonicalUsage {
            input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
        .unwrap_or_default();

    let mut content: Vec<CanonicalContent> = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            extract_output_item(item, &mut content);
        }
    }

    let stop_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_openai)
        .or(Some(CanonicalStopReason::EndTurn));

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    }
}

fn extract_output_item(item: &Value, content: &mut Vec<CanonicalContent>) {
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "message" => extract_message_parts(item, content),
        "function_call" => extract_function_call(item, content),
        "reasoning" => {
            if let Some(thinking) = extract_reasoning(item) {
                content.push(thinking);
            }
        },
        _ => {},
    }
}

fn extract_message_parts(item: &Value, content: &mut Vec<CanonicalContent>) {
    let Some(parts) = item.get("content").and_then(Value::as_array) else {
        return;
    };
    for p in parts {
        let ptype = p.get("type").and_then(Value::as_str).unwrap_or("");
        if matches!(ptype, "output_text" | "text") {
            if let Some(text) = p.get("text").and_then(Value::as_str) {
                content.push(CanonicalContent::Text(text.to_string()));
            }
        }
    }
}

fn extract_function_call(item: &Value, content: &mut Vec<CanonicalContent>) {
    let id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let args = item
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let input: Value = serde_json::from_str(args).unwrap_or(Value::Object(Map::new()));
    content.push(CanonicalContent::ToolUse { id, name, input });
}

fn extract_reasoning(item: &Value) -> Option<CanonicalContent> {
    let text = item
        .get("summary")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    if text.is_empty() {
        None
    } else {
        Some(CanonicalContent::Thinking {
            text,
            signature: None,
        })
    }
}
