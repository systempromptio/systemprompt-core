//! `OpenAI` Chat Completions request rendering from the canonical model.
//!
//! Wire idiosyncrasies encoded here: the output-token limit is emitted as
//! `max_completion_tokens` (gpt-5 / o-series reject the legacy `max_tokens`)
//! and sized via [`super::output_token_ceiling`] so reasoning models are not
//! starved of budget, and streamed usage requires
//! `stream_options.include_usage`.

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::{Map, Value, json};

use crate::profile::WireProtocol;
use crate::schema::SchemaSanitizer;
use crate::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource,
    ResponseFormat, Role,
};

pub fn build_request_body(
    request: &CanonicalRequest,
    upstream_model: &str,
    max_output_tokens: Option<u32>,
) -> Value {
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
    obj.insert(
        "max_completion_tokens".into(),
        Value::from(super::output_token_ceiling(
            request,
            upstream_model,
            max_output_tokens,
        )),
    );
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
        let sanitizer = SchemaSanitizer::new(WireProtocol::OpenAiChat.schema_capabilities());
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": sanitizer.sanitize(t.input_schema.clone()),
                    },
                })
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_chat(tc));
    }
    if let Some(p) = request.presence_penalty {
        obj.insert("presence_penalty".into(), json!(p));
    }
    if let Some(p) = request.frequency_penalty {
        obj.insert("frequency_penalty".into(), json!(p));
    }
    if let Some(effort) = request.reasoning_effort {
        obj.insert(
            "reasoning_effort".into(),
            Value::String(effort.as_str().to_owned()),
        );
    }
    if let Some(format) = &request.response_format {
        obj.insert("response_format".into(), response_format_to_chat(format));
    }
    Value::Object(obj)
}

fn response_format_to_chat(format: &ResponseFormat) -> Value {
    match format {
        ResponseFormat::JsonObject => json!({ "type": "json_object" }),
        ResponseFormat::JsonSchema {
            name,
            schema,
            strict,
        } => json!({
            "type": "json_schema",
            "json_schema": { "name": name, "strict": strict, "schema": schema },
        }),
    }
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
            CanonicalContent::ToolUse {
                id, name, input, ..
            } => {
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
        CanonicalContent::Image(src) => {
            let (url, detail) = match src {
                ImageSource::Url { url, detail } => (url.clone(), *detail),
                ImageSource::Base64 {
                    media_type,
                    data,
                    detail,
                } => (format!("data:{media_type};base64,{data}"), *detail),
            };
            let mut image_url = Map::new();
            image_url.insert("url".into(), Value::String(url));
            if let Some(d) = detail {
                image_url.insert("detail".into(), Value::String(d.as_str().to_owned()));
            }
            Some(json!({ "type": "image_url", "image_url": Value::Object(image_url) }))
        },
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
