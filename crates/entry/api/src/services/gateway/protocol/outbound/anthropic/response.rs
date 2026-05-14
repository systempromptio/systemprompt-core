// JSON: protocol boundary — Anthropic Messages outbound wire format is dynamic
// JSON.
use serde_json::Value;

use super::super::super::canonical::{CanonicalContent, ImageSource};
use super::super::super::canonical_response::{
    CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

pub(super) fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_string();
    let stop_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_anthropic);
    let usage = value.get("usage").map_or(
        CanonicalUsage {
            input_tokens: 0,
            output_tokens: 0,
        },
        |u| CanonicalUsage {
            input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
        },
    );

    let mut content: Vec<CanonicalContent> = Vec::new();
    if let Some(arr) = value.get("content").and_then(Value::as_array) {
        for block in arr {
            if let Some(part) = parse_content_block(block) {
                content.push(part);
            }
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

fn parse_content_block(value: &Value) -> Option<CanonicalContent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "text" => Some(CanonicalContent::Text(
            value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        )),
        "thinking" => Some(CanonicalContent::Thinking {
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
        "tool_use" => Some(CanonicalContent::ToolUse {
            id: value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "image" => parse_image_block(value),
        _ => None,
    }
}

fn parse_image_block(value: &Value) -> Option<CanonicalContent> {
    let src = value.get("source")?;
    let stype = src.get("type").and_then(Value::as_str)?;
    match stype {
        "base64" => Some(CanonicalContent::Image(ImageSource::Base64 {
            media_type: src
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_string(),
            data: src
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })),
        "url" => Some(CanonicalContent::Image(ImageSource::Url(
            src.get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ))),
        _ => None,
    }
}
