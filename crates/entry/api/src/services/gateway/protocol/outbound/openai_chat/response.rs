// JSON: protocol boundary — OpenAI Chat Completions outbound wire format is
// dynamic JSON.
use serde_json::{Map, Value};
use uuid::Uuid;

use super::super::super::canonical::CanonicalContent;
use super::super::super::canonical_response::{
    CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

pub(super) fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("msg_{}", Uuid::new_v4().simple()), str::to_owned);
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_owned();
    let usage = value.get("usage").map_or(
        CanonicalUsage {
            input_tokens: 0,
            output_tokens: 0,
        },
        |u| CanonicalUsage {
            input_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: u
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
        },
    );

    let mut content: Vec<CanonicalContent> = Vec::new();
    let mut stop_reason = None;
    if let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    {
        stop_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(CanonicalStopReason::from_openai);
        if let Some(msg) = choice.get("message") {
            extract_message_content(msg, &mut content);
        }
    }

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    }
}

fn extract_message_content(msg: &Value, content: &mut Vec<CanonicalContent>) {
    if let Some(text) = msg.get("content").and_then(Value::as_str) {
        if !text.is_empty() {
            content.push(CanonicalContent::Text(text.to_owned()));
        }
    }
    if let Some(tool_calls) = msg.get("tool_calls").and_then(Value::as_array) {
        for tc in tool_calls {
            let id = tc
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let func = tc.get("function").unwrap_or(&Value::Null);
            let name = func
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let args = func
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input: Value = serde_json::from_str(args).unwrap_or(Value::Object(Map::new()));
            content.push(CanonicalContent::ToolUse { id, name, input });
        }
    }
}
