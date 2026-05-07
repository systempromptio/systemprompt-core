use bytes::Bytes;
use http::StatusCode;
use serde_json::{Map, Value, json};

use super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, Role, ThinkingConfig,
};
use super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, ContentBlockKind,
};
use super::{InboundAdapter, InboundParseError};

#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicMessagesInbound;

impl InboundAdapter for AnthropicMessagesInbound {
    fn wire_name(&self) -> &'static str {
        "anthropic.messages"
    }

    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError> {
        let value: Value =
            serde_json::from_slice(raw).map_err(|e| InboundParseError::InvalidJson(e.to_string()))?;
        parse(&value)
    }

    fn render_response(&self, response: &CanonicalResponse) -> Bytes {
        let value = render_response_value(response);
        Bytes::from(serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()))
    }

    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes> {
        render_event_frame(event, model)
    }

    fn render_error(&self, _status: StatusCode, message: &str) -> Bytes {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!(
            "{{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
        );
        Bytes::from(body)
    }
}

fn parse(value: &Value) -> Result<CanonicalRequest, InboundParseError> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .ok_or(InboundParseError::MissingField("model"))?
        .to_string();
    let max_tokens = value
        .get("max_tokens")
        .and_then(Value::as_u64)
        .ok_or(InboundParseError::MissingField("max_tokens"))? as u32;

    let system = value
        .get("system")
        .map(parse_system)
        .transpose()?
        .flatten();

    let messages = value
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(InboundParseError::MissingField("messages"))?
        .iter()
        .map(parse_message)
        .collect::<Result<Vec<_>, _>>()?;

    let temperature = value.get("temperature").and_then(Value::as_f64).map(|v| v as f32);
    let top_p = value.get("top_p").and_then(Value::as_f64).map(|v| v as f32);
    let top_k = value.get("top_k").and_then(Value::as_i64).map(|v| v as i32);

    let stop_sequences = value
        .get("stop_sequences")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let tools = value
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(parse_tool).collect::<Vec<_>>())
        .unwrap_or_default();

    let tool_choice = value.get("tool_choice").and_then(parse_tool_choice);

    let stream = value.get("stream").and_then(Value::as_bool).unwrap_or(false);

    let thinking = value.get("thinking").map(parse_thinking);

    let metadata = value.get("metadata").cloned();

    Ok(CanonicalRequest {
        model,
        system,
        messages,
        max_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        tools,
        tool_choice,
        stream,
        thinking,
        metadata,
    })
}

fn parse_system(value: &Value) -> Result<Option<String>, InboundParseError> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) if s.is_empty() => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        Value::Array(arr) => {
            let joined = arr
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(if joined.is_empty() {
                None
            } else {
                Some(joined)
            })
        },
        other => Err(InboundParseError::Unsupported {
            field: "system",
            detail: format!("expected string or array, got {other}"),
        }),
    }
}

fn parse_message(value: &Value) -> Result<CanonicalMessage, InboundParseError> {
    let role_str = value
        .get("role")
        .and_then(Value::as_str)
        .ok_or(InboundParseError::MissingField("messages[].role"))?;
    let role = match role_str {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" => Role::System,
        "tool" => Role::Tool,
        other => {
            return Err(InboundParseError::Unsupported {
                field: "messages[].role",
                detail: other.to_string(),
            });
        },
    };
    let content_value = value
        .get("content")
        .ok_or(InboundParseError::MissingField("messages[].content"))?;
    let content = parse_content(content_value)?;
    Ok(CanonicalMessage { role, content })
}

fn parse_content(value: &Value) -> Result<Vec<CanonicalContent>, InboundParseError> {
    match value {
        Value::String(s) => Ok(vec![CanonicalContent::Text(s.clone())]),
        Value::Array(blocks) => blocks.iter().map(parse_content_block).collect(),
        other => Err(InboundParseError::Unsupported {
            field: "messages[].content",
            detail: format!("unexpected shape: {other}"),
        }),
    }
}

fn parse_content_block(value: &Value) -> Result<CanonicalContent, InboundParseError> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("text");
    match kind {
        "text" => Ok(CanonicalContent::Text(
            value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        )),
        "image" => parse_image(value),
        "tool_use" => Ok(CanonicalContent::ToolUse {
            id: value.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "tool_result" => {
            let inner = value.get("content").map(parse_tool_result_content).unwrap_or_else(Vec::new);
            Ok(CanonicalContent::ToolResult {
                tool_use_id: value
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                content: inner,
                is_error: value
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        },
        "thinking" => Ok(CanonicalContent::Thinking {
            text: value
                .get("thinking")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        other => Err(InboundParseError::Unsupported {
            field: "messages[].content[].type",
            detail: other.to_string(),
        }),
    }
}

fn parse_tool_result_content(value: &Value) -> Vec<CanonicalContent> {
    match value {
        Value::String(s) => vec![CanonicalContent::Text(s.clone())],
        Value::Array(arr) => arr.iter().filter_map(|v| parse_content_block(v).ok()).collect(),
        _ => Vec::new(),
    }
}

fn parse_image(value: &Value) -> Result<CanonicalContent, InboundParseError> {
    let source = value
        .get("source")
        .ok_or(InboundParseError::MissingField("image.source"))?;
    let kind = source.get("type").and_then(Value::as_str).unwrap_or("base64");
    match kind {
        "base64" => Ok(CanonicalContent::Image(ImageSource::Base64 {
            media_type: source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_string(),
            data: source
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })),
        "url" => Ok(CanonicalContent::Image(ImageSource::Url(
            source
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ))),
        other => Err(InboundParseError::Unsupported {
            field: "image.source.type",
            detail: other.to_string(),
        }),
    }
}

fn parse_tool(value: &Value) -> CanonicalTool {
    CanonicalTool {
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        input_schema: value
            .get("input_schema")
            .cloned()
            .unwrap_or(Value::Object(Map::new())),
    }
}

fn parse_tool_choice(value: &Value) -> Option<CanonicalToolChoice> {
    if let Some(s) = value.as_str() {
        return match s {
            "auto" => Some(CanonicalToolChoice::Auto),
            "any" => Some(CanonicalToolChoice::Any),
            "none" => Some(CanonicalToolChoice::None),
            "required" => Some(CanonicalToolChoice::Required),
            _ => None,
        };
    }
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "auto" => Some(CanonicalToolChoice::Auto),
        "any" => Some(CanonicalToolChoice::Any),
        "none" => Some(CanonicalToolChoice::None),
        "required" => Some(CanonicalToolChoice::Required),
        "tool" => value
            .get("name")
            .and_then(Value::as_str)
            .map(|n| CanonicalToolChoice::Tool(n.to_string())),
        _ => None,
    }
}

fn parse_thinking(value: &Value) -> ThinkingConfig {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let enabled = kind == "enabled";
    let budget_tokens = value
        .get("budget_tokens")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    ThinkingConfig {
        enabled,
        budget_tokens,
    }
}

pub fn render_response_value(response: &CanonicalResponse) -> Value {
    let content: Vec<Value> = response
        .content
        .iter()
        .filter_map(content_to_anthropic_block)
        .collect();
    json!({
        "id": response.id,
        "type": "message",
        "role": "assistant",
        "model": response.model,
        "content": content,
        "stop_reason": response.stop_reason.map(|r| r.anthropic_str()),
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
        },
    })
}

pub fn content_to_anthropic_block(part: &CanonicalContent) -> Option<Value> {
    match part {
        CanonicalContent::Text(t) => Some(json!({ "type": "text", "text": t })),
        CanonicalContent::Thinking { text, signature } => {
            let mut obj = json!({ "type": "thinking", "thinking": text });
            if let Some(sig) = signature {
                obj.as_object_mut()
                    .expect("just constructed")
                    .insert("signature".into(), Value::String(sig.clone()));
            }
            Some(obj)
        },
        CanonicalContent::ToolUse { id, name, input } => Some(json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        })),
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let inner: Vec<Value> = content
                .iter()
                .filter_map(content_to_anthropic_block)
                .collect();
            Some(json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "is_error": is_error,
                "content": inner,
            }))
        },
        CanonicalContent::Image(src) => match src {
            ImageSource::Base64 { media_type, data } => Some(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": data,
                },
            })),
            ImageSource::Url(u) => Some(json!({
                "type": "image",
                "source": { "type": "url", "url": u },
            })),
        },
    }
}

fn render_event_frame(event: &CanonicalEvent, model: &str) -> Option<Bytes> {
    let value = match event {
        CanonicalEvent::MessageStart { id, model: m, usage } => json!({
            "type": "message_start",
            "message": {
                "id": id,
                "type": "message",
                "role": "assistant",
                "model": if m.is_empty() { model } else { m },
                "content": [],
                "stop_reason": Value::Null,
                "stop_sequence": Value::Null,
                "usage": {
                    "input_tokens": usage.input_tokens,
                    "output_tokens": usage.output_tokens,
                },
            },
        }),
        CanonicalEvent::ContentBlockStart { index, block } => {
            let block_value = match block {
                ContentBlockKind::Text => json!({ "type": "text", "text": "" }),
                ContentBlockKind::Thinking { signature } => {
                    let mut v = json!({ "type": "thinking", "thinking": "" });
                    if let Some(s) = signature {
                        v.as_object_mut()
                            .expect("just constructed")
                            .insert("signature".into(), Value::String(s.clone()));
                    }
                    v
                },
                ContentBlockKind::ToolUse { id, name } => json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": {},
                }),
            };
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": block_value,
            })
        },
        CanonicalEvent::TextDelta { index, text } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "text_delta", "text": text },
        }),
        CanonicalEvent::ThinkingDelta { index, text } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "thinking_delta", "thinking": text },
        }),
        CanonicalEvent::ToolUseDelta { index, partial_json } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "input_json_delta", "partial_json": partial_json },
        }),
        CanonicalEvent::ContentBlockStop { index } => json!({
            "type": "content_block_stop",
            "index": index,
        }),
        CanonicalEvent::UsageDelta(usage) => json!({
            "type": "message_delta",
            "delta": {},
            "usage": {
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
            },
        }),
        CanonicalEvent::MessageStop { stop_reason } => {
            let rendered = json!({
                "type": "message_delta",
                "delta": { "stop_reason": stop_reason.map(CanonicalStopReason::anthropic_str) },
                "usage": { "output_tokens": 0 },
            });
            return Some(Bytes::from(format!(
                "event: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
                serde_json::to_string(&rendered).unwrap_or_else(|_| "{}".into())
            )));
        },
        CanonicalEvent::Error(msg) => {
            let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
            return Some(Bytes::from(format!(
                "event: error\ndata: {{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}\n\n"
            )));
        },
    };
    let event_name = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message")
        .to_string();
    Some(Bytes::from(format!(
        "event: {event_name}\ndata: {}\n\n",
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
    )))
}
