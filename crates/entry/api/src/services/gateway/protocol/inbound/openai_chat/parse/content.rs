//! Message-content parsing: user parts, image sources, assistant tool calls,
//! and text flattening.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::{Map, Value};

use super::super::super::super::canonical::{
    CanonicalContent, CanonicalMessage, ImageDetail, ImageSource, Role,
};

pub(super) fn parse_user_content(value: Option<&Value>) -> Vec<CanonicalContent> {
    match value {
        Some(Value::String(s)) => vec![CanonicalContent::Text(s.clone())],
        Some(Value::Array(parts)) => parts.iter().filter_map(parse_user_part).collect(),
        _ => Vec::new(),
    }
}

fn parse_user_part(part: &Value) -> Option<CanonicalContent> {
    match part.get("type").and_then(Value::as_str)? {
        "text" => part
            .get("text")
            .and_then(Value::as_str)
            .map(|t| CanonicalContent::Text(t.to_owned())),
        "image_url" => {
            let image = part.get("image_url")?;
            let url = image.get("url").and_then(Value::as_str)?.to_owned();
            let detail = image
                .get("detail")
                .and_then(Value::as_str)
                .and_then(parse_image_detail);
            Some(CanonicalContent::Image(parse_image_source(url, detail)))
        },
        _ => None,
    }
}

fn parse_image_source(url: String, detail: Option<ImageDetail>) -> ImageSource {
    // Why: data URIs must round-trip to providers (Anthropic) that only accept
    // base64 source blocks, so split them back apart here.
    if let Some(rest) = url.strip_prefix("data:")
        && let Some((media_type, data)) = rest.split_once(";base64,")
    {
        return ImageSource::Base64 {
            media_type: media_type.to_owned(),
            data: data.to_owned(),
            detail,
        };
    }
    ImageSource::Url { url, detail }
}

fn parse_image_detail(s: &str) -> Option<ImageDetail> {
    match s {
        "auto" => Some(ImageDetail::Auto),
        "low" => Some(ImageDetail::Low),
        "high" => Some(ImageDetail::High),
        _ => None,
    }
}

pub(super) fn parse_assistant_message(msg: &Value) -> CanonicalMessage {
    let mut content: Vec<CanonicalContent> = Vec::new();
    let text = flatten_content_text(msg.get("content"));
    if !text.is_empty() {
        content.push(CanonicalContent::Text(text));
    }
    if let Some(calls) = msg.get("tool_calls").and_then(Value::as_array) {
        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let function = call.get("function");
            let name = function
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let args = function
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            // JSON: tool-call arguments are a user-defined schema instance; the
            // canonical model carries them as an opaque JSON value.
            let input: Value =
                serde_json::from_str(args).unwrap_or_else(|_| Value::Object(Map::new()));
            content.push(CanonicalContent::ToolUse {
                id,
                name,
                input,
                signature: None,
            });
        }
    }
    CanonicalMessage {
        role: Role::Assistant,
        content,
    }
}

pub(super) fn flatten_content_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
