//! Canonical-to-Anthropic content block rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use crate::wire::canonical::{CanonicalContent, CanonicalMessage, ImageSource, Role};

// Why: the real Anthropic API rejects unknown keys in content blocks, while
// the gateway's own client relies on its vendor-extension fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockAudience {
    Client,
    Upstream,
}

#[must_use]
pub fn content_to_anthropic_block(part: &CanonicalContent) -> Value {
    block_for_audience(part, BlockAudience::Client)
}

pub(super) fn block_for_audience(part: &CanonicalContent, audience: BlockAudience) -> Value {
    match part {
        CanonicalContent::Text(t) => json!({ "type": "text", "text": t }),
        CanonicalContent::Thinking {
            text, signature, ..
        } => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("thinking".into()));
            obj.insert("thinking".into(), Value::String(text.clone()));
            if let Some(sig) = signature {
                obj.insert("signature".into(), Value::String(sig.clone()));
            }
            Value::Object(obj)
        },
        CanonicalContent::ToolUse {
            id,
            name,
            input,
            signature,
        } => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("tool_use".into()));
            obj.insert("id".into(), Value::String(id.clone()));
            obj.insert("name".into(), Value::String(name.clone()));
            obj.insert("input".into(), input.clone());
            if audience == BlockAudience::Client
                && let Some(sig) = signature
            {
                obj.insert("signature".into(), Value::String(sig.clone()));
            }
            Value::Object(obj)
        },
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured_content,
            meta,
        } => {
            let inner: Vec<Value> = content
                .iter()
                .map(|p| block_for_audience(p, audience))
                .collect();
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("tool_result".into()));
            obj.insert("tool_use_id".into(), Value::String(tool_use_id.clone()));
            obj.insert("is_error".into(), Value::Bool(*is_error));
            obj.insert("content".into(), Value::Array(inner));
            if audience == BlockAudience::Client {
                if let Some(sc) = structured_content {
                    obj.insert("structuredContent".into(), sc.clone());
                }
                if let Some(m) = meta {
                    obj.insert("_meta".into(), m.clone());
                }
            }
            Value::Object(obj)
        },
        CanonicalContent::Image(src) => match src {
            ImageSource::Base64 {
                media_type, data, ..
            } => json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media_type, "data": data },
            }),
            ImageSource::Url { url, .. } => json!({
                "type": "image",
                "source": { "type": "url", "url": url },
            }),
        },
    }
}

pub(super) fn canonical_message_to_anthropic(
    msg: &CanonicalMessage,
    audience: BlockAudience,
) -> Option<Value> {
    let role = match msg.role {
        Role::Assistant => "assistant",
        Role::User | Role::Tool | Role::System => "user",
    };
    let content: Vec<Value> = msg
        .content
        .iter()
        .filter(|part| {
            // Why: Anthropic 400s on a replayed thinking block without its
            // signature; history without the block is valid and merely loses
            // continuity.
            audience == BlockAudience::Client
                || !matches!(
                    part,
                    CanonicalContent::Thinking {
                        signature: None,
                        ..
                    }
                )
        })
        .map(|part| block_for_audience(part, audience))
        .collect();
    if content.is_empty() {
        return None;
    }
    Some(json!({ "role": role, "content": content }))
}
