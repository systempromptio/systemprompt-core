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

const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiResponsesInbound;

impl InboundAdapter for OpenAiResponsesInbound {
    fn wire_name(&self) -> &'static str {
        "openai.responses"
    }

    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError> {
        let value: Value =
            serde_json::from_slice(raw).map_err(|e| InboundParseError::InvalidJson(e.to_string()))?;
        parse(&value)
    }

    fn render_response(&self, response: &CanonicalResponse) -> Bytes {
        let value = render_response_object(response);
        Bytes::from(serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()))
    }

    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes> {
        render_event_frame(event, model)
    }

    fn render_error(&self, _status: StatusCode, message: &str) -> Bytes {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!(
            "{{\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
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
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);

    let system = value
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let messages = value
        .get("input")
        .map(parse_input)
        .transpose()?
        .unwrap_or_default();

    let temperature = value.get("temperature").and_then(Value::as_f64).map(|v| v as f32);
    let top_p = value.get("top_p").and_then(Value::as_f64).map(|v| v as f32);

    let stop_sequences = value
        .get("stop")
        .and_then(|v| match v {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|x| x.as_str().map(ToString::to_string))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();

    let tools = value
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_tool).collect::<Vec<_>>())
        .unwrap_or_default();

    let tool_choice = value.get("tool_choice").and_then(parse_tool_choice);

    let stream = value.get("stream").and_then(Value::as_bool).unwrap_or(false);

    let thinking = value.get("reasoning").map(parse_reasoning);

    let metadata = value.get("metadata").cloned();

    Ok(CanonicalRequest {
        model,
        system,
        messages,
        max_tokens,
        temperature,
        top_p,
        top_k: None,
        stop_sequences,
        tools,
        tool_choice,
        stream,
        thinking,
        metadata,
    })
}

fn parse_input(value: &Value) -> Result<Vec<CanonicalMessage>, InboundParseError> {
    let arr = match value {
        Value::String(s) => {
            return Ok(vec![CanonicalMessage {
                role: Role::User,
                content: vec![CanonicalContent::Text(s.clone())],
            }]);
        },
        Value::Array(a) => a,
        other => {
            return Err(InboundParseError::Unsupported {
                field: "input",
                detail: format!("expected string or array, got {other}"),
            });
        },
    };

    let mut messages: Vec<CanonicalMessage> = Vec::new();

    for item in arr {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or("message");
        match kind {
            "message" => messages.push(parse_message_item(item)?),
            "function_call" => {
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
                let arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("{}");
                let input: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
                messages.push(CanonicalMessage {
                    role: Role::Assistant,
                    content: vec![CanonicalContent::ToolUse { id, name, input }],
                });
            },
            "function_call_output" => {
                let tool_use_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let output_text = item
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                messages.push(CanonicalMessage {
                    role: Role::Tool,
                    content: vec![CanonicalContent::ToolResult {
                        tool_use_id,
                        content: vec![CanonicalContent::Text(output_text)],
                        is_error: false,
                    }],
                });
            },
            "reasoning" => {
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
                if !text.is_empty() {
                    messages.push(CanonicalMessage {
                        role: Role::Assistant,
                        content: vec![CanonicalContent::Thinking {
                            text,
                            signature: None,
                        }],
                    });
                }
            },
            other => {
                return Err(InboundParseError::Unsupported {
                    field: "input[].type",
                    detail: other.to_string(),
                });
            },
        }
    }

    Ok(messages)
}

fn parse_message_item(value: &Value) -> Result<CanonicalMessage, InboundParseError> {
    let role_str = value.get("role").and_then(Value::as_str).unwrap_or("user");
    let role = match role_str {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" | "developer" => Role::System,
        other => {
            return Err(InboundParseError::Unsupported {
                field: "input[].role",
                detail: other.to_string(),
            });
        },
    };
    let content_value = value.get("content").unwrap_or(&Value::Null);
    let content = match content_value {
        Value::String(s) => vec![CanonicalContent::Text(s.clone())],
        Value::Array(parts) => parts.iter().filter_map(parse_content_part).collect(),
        Value::Null => Vec::new(),
        other => {
            return Err(InboundParseError::Unsupported {
                field: "input[].content",
                detail: format!("unexpected: {other}"),
            });
        },
    };
    Ok(CanonicalMessage { role, content })
}

fn parse_content_part(value: &Value) -> Option<CanonicalContent> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "input_text" | "output_text" | "text" => Some(CanonicalContent::Text(
            value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        )),
        "input_image" => {
            let url = value.get("image_url").and_then(Value::as_str)?;
            Some(CanonicalContent::Image(ImageSource::Url(url.to_string())))
        },
        _ => None,
    }
}

fn parse_tool(value: &Value) -> Option<CanonicalTool> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("function");
    if kind != "function" {
        return None;
    }
    let name = value.get("name").and_then(Value::as_str)?;
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let parameters = value
        .get("parameters")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    Some(CanonicalTool {
        name: name.to_string(),
        description,
        input_schema: parameters,
    })
}

fn parse_tool_choice(value: &Value) -> Option<CanonicalToolChoice> {
    if let Some(s) = value.as_str() {
        return match s {
            "auto" => Some(CanonicalToolChoice::Auto),
            "none" => Some(CanonicalToolChoice::None),
            "required" => Some(CanonicalToolChoice::Required),
            _ => None,
        };
    }
    let kind = value.get("type").and_then(Value::as_str)?;
    if kind == "function" {
        return value
            .get("name")
            .and_then(Value::as_str)
            .map(|n| CanonicalToolChoice::Tool(n.to_string()));
    }
    None
}

fn parse_reasoning(value: &Value) -> ThinkingConfig {
    let effort = value.get("effort").and_then(Value::as_str).unwrap_or("");
    let enabled = !effort.is_empty();
    let budget_tokens = match effort {
        "low" => Some(1024),
        "medium" => Some(4096),
        "high" => Some(16384),
        _ => None,
    };
    ThinkingConfig {
        enabled,
        budget_tokens,
    }
}

fn render_response_object(response: &CanonicalResponse) -> Value {
    let mut output: Vec<Value> = Vec::new();
    let mut text_parts: Vec<Value> = Vec::new();

    for part in &response.content {
        match part {
            CanonicalContent::Text(t) => {
                text_parts.push(json!({ "type": "output_text", "text": t, "annotations": [] }));
            },
            CanonicalContent::ToolUse { id, name, input } => {
                let arguments = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
                output.push(json!({
                    "type": "function_call",
                    "id": format!("fc_{id}"),
                    "call_id": id,
                    "name": name,
                    "arguments": arguments,
                    "status": "completed",
                }));
            },
            CanonicalContent::Thinking { text, .. } => {
                output.push(json!({
                    "type": "reasoning",
                    "id": format!("rs_{}", response.id),
                    "summary": [{ "type": "summary_text", "text": text }],
                }));
            },
            CanonicalContent::Image(_) | CanonicalContent::ToolResult { .. } => {},
        }
    }

    if !text_parts.is_empty() {
        output.insert(
            0,
            json!({
                "type": "message",
                "id": format!("msg_{}", response.id),
                "status": "completed",
                "role": "assistant",
                "content": text_parts,
            }),
        );
    }

    json!({
        "id": response.id,
        "object": "response",
        "created_at": current_unix_ts(),
        "status": "completed",
        "model": response.model,
        "output": output,
        "usage": {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
            "total_tokens": response.usage.input_tokens + response.usage.output_tokens,
        },
        "stop_reason": response.stop_reason.map(|r| r.openai_str()),
    })
}

fn current_unix_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn render_event_frame(event: &CanonicalEvent, model: &str) -> Option<Bytes> {
    let (event_name, payload): (&str, Value) = match event {
        CanonicalEvent::MessageStart { id, model: m, usage } => (
            "response.created",
            json!({
                "type": "response.created",
                "response": {
                    "id": id,
                    "object": "response",
                    "created_at": current_unix_ts(),
                    "status": "in_progress",
                    "model": if m.is_empty() { model } else { m },
                    "output": [],
                    "usage": {
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                    },
                },
            }),
        ),
        CanonicalEvent::ContentBlockStart { index, block } => match block {
            ContentBlockKind::Text => (
                "response.output_item.added",
                json!({
                    "type": "response.output_item.added",
                    "output_index": index,
                    "item": {
                        "type": "message",
                        "id": format!("msg_{index}"),
                        "status": "in_progress",
                        "role": "assistant",
                        "content": [],
                    },
                }),
            ),
            ContentBlockKind::ToolUse { id, name } => (
                "response.output_item.added",
                json!({
                    "type": "response.output_item.added",
                    "output_index": index,
                    "item": {
                        "type": "function_call",
                        "id": format!("fc_{id}"),
                        "call_id": id,
                        "name": name,
                        "arguments": "",
                        "status": "in_progress",
                    },
                }),
            ),
            ContentBlockKind::Thinking { .. } => (
                "response.output_item.added",
                json!({
                    "type": "response.output_item.added",
                    "output_index": index,
                    "item": {
                        "type": "reasoning",
                        "id": format!("rs_{index}"),
                        "summary": [],
                    },
                }),
            ),
        },
        CanonicalEvent::TextDelta { index, text } => (
            "response.output_text.delta",
            json!({
                "type": "response.output_text.delta",
                "output_index": index,
                "content_index": 0,
                "delta": text,
            }),
        ),
        CanonicalEvent::ThinkingDelta { index, text } => (
            "response.reasoning_summary_text.delta",
            json!({
                "type": "response.reasoning_summary_text.delta",
                "output_index": index,
                "summary_index": 0,
                "delta": text,
            }),
        ),
        CanonicalEvent::ToolUseDelta { index, partial_json } => (
            "response.function_call_arguments.delta",
            json!({
                "type": "response.function_call_arguments.delta",
                "output_index": index,
                "delta": partial_json,
            }),
        ),
        CanonicalEvent::ContentBlockStop { index } => (
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "output_index": index,
            }),
        ),
        CanonicalEvent::UsageDelta(_) => return None,
        CanonicalEvent::MessageStop { stop_reason } => (
            "response.completed",
            json!({
                "type": "response.completed",
                "response": {
                    "status": "completed",
                    "stop_reason": stop_reason.map(|r| r.openai_str()),
                },
            }),
        ),
        CanonicalEvent::Error(msg) => {
            let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
            return Some(Bytes::from(format!(
                "event: response.failed\ndata: {{\"type\":\"response.failed\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}\n\n"
            )));
        },
    };
    Some(Bytes::from(format!(
        "event: {event_name}\ndata: {}\n\n",
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into())
    )))
}
