// JSON: protocol boundary — OpenAI Chat Completions outbound wire format is
// dynamic JSON.
use serde_json::{Map, Value, json};

use super::super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource, Role,
};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(sys) = &request.system {
        messages.push(json!({ "role": "system", "content": sys }));
    }
    for msg in &request.messages {
        messages.extend(canonical_message_to_chat(msg));
    }

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_owned()));
    obj.insert("messages".into(), Value::Array(messages));
    obj.insert("max_tokens".into(), Value::from(request.max_tokens));
    if let Some(t) = request.temperature {
        obj.insert("temperature".into(), json!(t));
    }
    if let Some(p) = request.top_p {
        obj.insert("top_p".into(), json!(p));
    }
    if !request.stop_sequences.is_empty() {
        obj.insert("stop".into(), json!(request.stop_sequences));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
        obj.insert("stream_options".into(), json!({ "include_usage": true }));
    }
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    },
                })
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_chat(tc));
    }
    Value::Object(obj)
}

fn canonical_message_to_chat(msg: &CanonicalMessage) -> Vec<Value> {
    match msg.role {
        Role::System => vec![json!({
            "role": "system",
            "content": flatten_text(&msg.content),
        })],
        Role::User => render_user_message(&msg.content),
        Role::Assistant => render_assistant_message(&msg.content),
        Role::Tool => msg
            .content
            .iter()
            .filter_map(|c| match c {
                CanonicalContent::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } => Some(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": flatten_text(content),
                })),
                _ => None,
            })
            .collect(),
    }
}

fn render_user_message(content: &[CanonicalContent]) -> Vec<Value> {
    let parts: Vec<Value> = content.iter().filter_map(content_to_chat_part).collect();
    if parts.iter().all(is_text_part) {
        let text = parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        vec![json!({ "role": "user", "content": text })]
    } else {
        vec![json!({ "role": "user", "content": parts })]
    }
}

fn render_assistant_message(content: &[CanonicalContent]) -> Vec<Value> {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    for part in content {
        match part {
            CanonicalContent::Text(t) => text.push_str(t),
            CanonicalContent::ToolUse { id, name, input } => {
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(input)
                            .unwrap_or_else(|_| "{}".into()),
                    },
                }));
            },
            _ => {},
        }
    }
    let mut obj = Map::new();
    obj.insert("role".into(), Value::String("assistant".into()));
    if text.is_empty() {
        obj.insert("content".into(), Value::Null);
    } else {
        obj.insert("content".into(), Value::String(text));
    }
    if !tool_calls.is_empty() {
        obj.insert("tool_calls".into(), Value::Array(tool_calls));
    }
    vec![Value::Object(obj)]
}

fn content_to_chat_part(part: &CanonicalContent) -> Option<Value> {
    match part {
        CanonicalContent::Text(t) => Some(json!({ "type": "text", "text": t })),
        CanonicalContent::Image(src) => Some(json!({
            "type": "image_url",
            "image_url": {
                "url": match src {
                    ImageSource::Url(u) => u.clone(),
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    },
                },
            },
        })),
        _ => None,
    }
}

fn is_text_part(v: &Value) -> bool {
    v.get("type").and_then(Value::as_str) == Some("text")
}

fn flatten_text(parts: &[CanonicalContent]) -> String {
    let mut out = String::new();
    for p in parts {
        if let CanonicalContent::Text(t) = p {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}

fn tool_choice_to_chat(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => Value::String("auto".into()),
        CanonicalToolChoice::None => Value::String("none".into()),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => {
            Value::String("required".into())
        },
        CanonicalToolChoice::Tool(name) => json!({
            "type": "function",
            "function": { "name": name },
        }),
    }
}
