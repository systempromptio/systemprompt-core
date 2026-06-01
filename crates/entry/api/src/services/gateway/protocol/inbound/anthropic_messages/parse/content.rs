//! Message and content-block parsing for the Anthropic Messages wire format.

// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::Value;

use crate::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, ImageSource, Role,
};
use crate::services::gateway::protocol::inbound::InboundParseError;

pub(super) fn parse_system(value: &Value) -> Result<Option<String>, InboundParseError> {
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

pub(super) fn parse_message(value: &Value) -> Result<CanonicalMessage, InboundParseError> {
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
                detail: other.to_owned(),
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
                .to_owned(),
        )),
        "image" => parse_image(value),
        "tool_use" => Ok(CanonicalContent::ToolUse {
            id: value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "tool_result" => {
            let inner = value
                .get("content")
                .map_or_else(Vec::new, parse_tool_result_content);
            Ok(CanonicalContent::ToolResult {
                tool_use_id: value
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
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
                .to_owned(),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        other => Err(InboundParseError::Unsupported {
            field: "messages[].content[].type",
            detail: other.to_owned(),
        }),
    }
}

fn parse_tool_result_content(value: &Value) -> Vec<CanonicalContent> {
    match value {
        Value::String(s) => vec![CanonicalContent::Text(s.clone())],
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| parse_content_block(v).ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_image(value: &Value) -> Result<CanonicalContent, InboundParseError> {
    let source = value
        .get("source")
        .ok_or(InboundParseError::MissingField("image.source"))?;
    let kind = source
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("base64");
    match kind {
        "base64" => Ok(CanonicalContent::Image(ImageSource::Base64 {
            media_type: source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_owned(),
            data: source
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            detail: None,
        })),
        "url" => Ok(CanonicalContent::Image(ImageSource::Url {
            url: source
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            detail: None,
        })),
        other => Err(InboundParseError::Unsupported {
            field: "image.source.type",
            detail: other.to_owned(),
        }),
    }
}
