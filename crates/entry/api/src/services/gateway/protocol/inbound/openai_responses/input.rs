//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::Value;

use super::super::super::canonical::{CanonicalContent, CanonicalMessage, ImageSource, Role};
use super::super::InboundParseError;

pub(super) fn parse_input(value: &Value) -> Result<Vec<CanonicalMessage>, InboundParseError> {
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
        let kind = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("message");
        match kind {
            "message" => messages.push(parse_message_item(item)?),
            "function_call" => messages.push(parse_function_call(item)),
            "function_call_output" => messages.push(parse_function_call_output(item)),
            "reasoning" => {
                if let Some(msg) = parse_reasoning_item(item) {
                    messages.push(msg);
                }
            },
            other => {
                return Err(InboundParseError::Unsupported {
                    field: "input[].type",
                    detail: other.to_owned(),
                });
            },
        }
    }
    Ok(messages)
}

fn parse_function_call(item: &Value) -> CanonicalMessage {
    let id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))
        .unwrap_or("")
        .to_owned();
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let arguments = item
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let input: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::ToolUse {
            id,
            name,
            input,
            signature: None,
        }],
    }
}

fn parse_function_call_output(item: &Value) -> CanonicalMessage {
    let tool_use_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let output_text = item
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    CanonicalMessage {
        role: Role::Tool,
        content: vec![CanonicalContent::ToolResult {
            tool_use_id,
            content: vec![CanonicalContent::Text(output_text)],
            is_error: false,
            structured_content: None,
            meta: None,
        }],
    }
}

fn parse_reasoning_item(item: &Value) -> Option<CanonicalMessage> {
    let text = item
        .get("summary")
        .and_then(Value::as_array)
        .map_or_else(String::new, |arr| {
            arr.iter()
                .filter_map(|v| v.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        });
    if text.is_empty() {
        return None;
    }
    Some(CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::Thinking {
            text,
            signature: None,
        }],
    })
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
                detail: other.to_owned(),
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
                .to_owned(),
        )),
        "input_image" => {
            let url = value.get("image_url").and_then(Value::as_str)?;
            Some(CanonicalContent::Image(ImageSource::Url {
                url: url.to_owned(),
                detail: None,
            }))
        },
        _ => None,
    }
}
