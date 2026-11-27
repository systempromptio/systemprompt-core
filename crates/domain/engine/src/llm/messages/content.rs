//! One neutral content block in its Messages form. Attachments the model cannot
//! take become a text note naming what was left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine as _;
use serde_json::{Map, Value, json};

use crate::model::{Attachment, Content};

// JSON: a Messages content block, or none for content the protocol drops.
pub(super) fn block(content: &Content, images: bool) -> Option<Value> {
    match content {
        Content::Text { text } => {
            (!text.trim().is_empty()).then(|| json!({"type": "text", "text": text}))
        },
        Content::Image { mime, data } => Some(image(mime, data, images)),
        Content::Document { mime, data, name } => Some(document(mime, data, name.as_deref())),
        Content::ToolCall { id, name, input } => {
            let input = if input.is_object() {
                input.clone()
            } else {
                Value::Object(Map::new())
            };
            Some(json!({"type": "tool_use", "id": tool_id(id), "name": name, "input": input}))
        },
        Content::ToolResult {
            id,
            output,
            is_error,
            attachments,
        } => Some(tool_result(id, output, *is_error, attachments, images)),
        Content::Reasoning { text, signature } => signature
            .as_ref()
            .map(|signature| json!({"type": "thinking", "thinking": text, "signature": signature})),
        Content::RedactedReasoning { data } => {
            Some(json!({"type": "redacted_thinking", "data": data}))
        },
    }
}

// JSON: a Messages `tool_result` block.
fn tool_result(
    id: &str,
    output: &str,
    is_error: bool,
    attachments: &[Attachment],
    images: bool,
) -> Value {
    let mut block = Map::new();
    block.insert("type".into(), "tool_result".into());
    block.insert("tool_use_id".into(), tool_id(id).into());
    if attachments.is_empty() {
        block.insert("content".into(), output.into());
    } else {
        let mut items = Vec::with_capacity(attachments.len() + 1);
        if !output.trim().is_empty() {
            items.push(json!({"type": "text", "text": output}));
        }
        for attachment in attachments {
            items.push(if attachment.mime.starts_with("image/") {
                image(&attachment.mime, &attachment.data, images)
            } else {
                document(
                    &attachment.mime,
                    &attachment.data,
                    attachment.name.as_deref(),
                )
            });
        }
        block.insert("content".into(), Value::Array(items));
    }
    if is_error {
        block.insert("is_error".into(), true.into());
    }
    Value::Object(block)
}

// JSON: a Messages `image` block, or the omission note when the model takes no
// images.
fn image(mime: &str, data: &str, accepted: bool) -> Value {
    if !accepted {
        return omitted(mime, None);
    }
    json!({"type": "image", "source": {"type": "base64", "media_type": mime, "data": data}})
}

// JSON: a Messages `document` block, or the omission note for a type it cannot
// carry.
fn document(mime: &str, data: &str, name: Option<&str>) -> Value {
    let source = if mime == "application/pdf" {
        json!({"type": "base64", "media_type": mime, "data": data})
    } else if mime.starts_with("text/") {
        match base64::engine::general_purpose::STANDARD.decode(data) {
            Ok(bytes) => json!({
                "type": "text",
                "media_type": "text/plain",
                "data": String::from_utf8_lossy(&bytes)
            }),
            Err(_) => return omitted(mime, name),
        }
    } else {
        return omitted(mime, name);
    };
    let mut block = json!({"type": "document", "source": source});
    if let (Some(name), Some(map)) = (name, block.as_object_mut()) {
        map.insert("title".into(), name.into());
    }
    block
}

// JSON: a Messages text block standing in for an attachment the model cannot
// take.
fn omitted(mime: &str, name: Option<&str>) -> Value {
    let label = name.map_or_else(|| mime.to_owned(), |n| format!("{n} ({mime})"));
    json!({
        "type": "text",
        "text": format!("[attachment {label} omitted: the model does not accept this type]")
    })
}

fn tool_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
