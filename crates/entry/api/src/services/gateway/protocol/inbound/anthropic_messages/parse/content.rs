//! Message and content-block parsing for the Anthropic Messages wire format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::Value;
use systemprompt_models::wire::anthropic::cache_control_from_anthropic;

use crate::services::gateway::protocol::canonical::{
    CacheControl, CanonicalContent, CanonicalMessage, ImageSource, Role, SystemBlock,
};
use crate::services::gateway::protocol::inbound::InboundParseError;

// Why: each system block keeps its own `cache_control`, so a rebuilt body
// puts the cache breakpoints back exactly where the client set them.
pub(super) fn parse_system(value: &Value) -> Result<Vec<SystemBlock>, InboundParseError> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::String(s) if s.is_empty() => Ok(Vec::new()),
        Value::String(s) => Ok(vec![SystemBlock::text(s.clone())]),
        Value::Array(arr) => Ok(arr
            .iter()
            .filter_map(|block| {
                let text = block.get("text").and_then(Value::as_str)?;
                Some(SystemBlock {
                    text: text.to_owned(),
                    cache_control: parse_cache_control(block),
                })
            })
            .collect()),
        other => Err(InboundParseError::Unsupported {
            field: "system",
            detail: format!("expected string or array, got {other}"),
        }),
    }
}

fn parse_cache_control(block: &Value) -> Option<CacheControl> {
    block
        .get("cache_control")
        .and_then(cache_control_from_anthropic)
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
        Value::String(s) => Ok(vec![CanonicalContent::text(s.clone())]),
        Value::Array(blocks) => {
            let mut out = Vec::with_capacity(blocks.len());
            for block in blocks {
                if let Some(content) = parse_content_block(block)? {
                    out.push(content);
                }
            }
            Ok(out)
        },
        other => Err(InboundParseError::Unsupported {
            field: "messages[].content",
            detail: format!("unexpected shape: {other}"),
        }),
    }
}

fn parse_content_block(value: &Value) -> Result<Option<CanonicalContent>, InboundParseError> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("text");
    match kind {
        "text" => Ok(Some(CanonicalContent::Text {
            text: value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            cache_control: parse_cache_control(value),
        })),
        "image" => parse_image(value).map(Some),
        "tool_use" => Ok(Some(CanonicalContent::ToolUse {
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
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
            cache_control: parse_cache_control(value),
        })),
        "tool_result" => {
            let inner = value
                .get("content")
                .map_or_else(Vec::new, parse_tool_result_content);
            Ok(Some(CanonicalContent::ToolResult {
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
                structured_content: value.get("structuredContent").cloned(),
                meta: value.get("_meta").cloned(),
                cache_control: parse_cache_control(value),
            }))
        },
        "thinking" => Ok(Some(CanonicalContent::Thinking {
            id: None,
            encrypted_content: None,
            text: value
                .get("thinking")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })),
        other => {
            tracing::debug!(
                block_type = %other,
                "dropped unmodelled content block from inbound message history"
            );
            Ok(None)
        },
    }
}

fn parse_tool_result_content(value: &Value) -> Vec<CanonicalContent> {
    match value {
        Value::String(s) => vec![CanonicalContent::text(s.clone())],
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| parse_content_block(v).ok().flatten())
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
    let source = match kind {
        "base64" => ImageSource::Base64 {
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
        },
        "url" => ImageSource::Url {
            url: source
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            detail: None,
        },
        other => {
            return Err(InboundParseError::Unsupported {
                field: "image.source.type",
                detail: other.to_owned(),
            });
        },
    };
    Ok(CanonicalContent::Image {
        source,
        cache_control: parse_cache_control(value),
    })
}
