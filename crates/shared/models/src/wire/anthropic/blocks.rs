//! Canonical-to-Anthropic content block rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use crate::wire::canonical::{
    CacheControl, CacheTtl, CanonicalContent, CanonicalMessage, ImageSource, Role,
};

// Why: Anthropic rejects unknown keys in upstream content blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockAudience {
    Client,
    Upstream,
}

#[must_use]
// JSON: Anthropic Messages API content block; upstream JSON is the contract.
pub fn content_to_anthropic_block(part: &CanonicalContent) -> Value {
    block_for_audience(part, BlockAudience::Client)
}

// JSON: Anthropic Messages API `cache_control` object; upstream JSON is the
// contract.
#[must_use]
pub fn cache_control_to_anthropic(cache_control: CacheControl) -> Value {
    let mut obj = Map::new();
    obj.insert("type".into(), Value::String("ephemeral".into()));
    if let Some(ttl) = cache_control.ttl {
        obj.insert("ttl".into(), Value::String(ttl.as_str().into()));
    }
    Value::Object(obj)
}

// JSON: Anthropic Messages API `cache_control` object; upstream JSON is the
// contract.
#[must_use]
pub fn cache_control_from_anthropic(value: &Value) -> Option<CacheControl> {
    let obj = value.as_object()?;
    if obj.get("type").and_then(Value::as_str) != Some("ephemeral") {
        return None;
    }
    let ttl = obj
        .get("ttl")
        .and_then(Value::as_str)
        .and_then(CacheTtl::parse);
    Some(CacheControl { ttl })
}

// JSON: Anthropic Messages API content block; upstream JSON is the contract.
pub(super) fn block_for_audience(part: &CanonicalContent, audience: BlockAudience) -> Value {
    let mut block = untagged_block(part, audience);
    if let (Some(obj), Some(cache_control)) = (block.as_object_mut(), part.cache_control()) {
        obj.insert(
            "cache_control".into(),
            cache_control_to_anthropic(cache_control),
        );
    }
    block
}

// JSON: Anthropic Messages API content block; upstream JSON is the contract.
fn untagged_block(part: &CanonicalContent, audience: BlockAudience) -> Value {
    match part {
        CanonicalContent::Text { text, .. } => json!({ "type": "text", "text": text }),
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
            ..
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
            ..
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
        CanonicalContent::Image { source, .. } => match source {
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
    // JSON: Anthropic Messages API content block; the upstream JSON is the contract.
) -> Option<Value> {
    let role = match msg.role {
        Role::Assistant => "assistant",
        Role::User | Role::Tool | Role::System => "user",
    };
    let content: Vec<Value> = msg
        .content
        .iter()
        .filter(|part| {
            // Why: Anthropic rejects replayed thinking blocks without their signatures.
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
