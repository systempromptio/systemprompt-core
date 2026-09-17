//! Turning what each vantage point saw into one MCP `CallToolResult`.
//!
//! The proxy tap and in-process executor already hold a wire result. A gateway
//! history carries a canonical `tool_result` block; a client hook carries
//! whatever the host put in `tool_response`, which for an MCP tool is the
//! result object itself and for a builtin tool is free-form. Everything is
//! normalised here so classification and storage see one shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{CallToolResult, ContentBlock, MetaObject};
use serde_json::Value as JsonValue;
use systemprompt_models::wire::canonical::{CanonicalContent, ImageSource};

/// An MCP-shaped value (`content` / `structuredContent` / `isError` /
/// `_meta`), if that is what the value is.
// JSON: the host's copy of the wire result, open-shaped until parsed.
#[must_use]
pub fn from_wire_value(value: &JsonValue) -> Option<CallToolResult> {
    if !value.is_object() {
        return None;
    }
    serde_json::from_value(value.clone()).ok()
}

/// A client hook's `tool_response`: the wire result when the host preserved
/// it, otherwise a string becomes one text block and any other value becomes
/// the tool's structured output.
// JSON: hook payloads are the host's own shape at the wire boundary.
#[must_use]
pub fn from_hook_response(value: &JsonValue) -> CallToolResult {
    if let Some(result) = from_wire_value(value) {
        return result;
    }
    match value {
        JsonValue::String(text) => CallToolResult::success(vec![ContentBlock::text(text.clone())]),
        JsonValue::Null => CallToolResult::success(Vec::new()),
        JsonValue::Array(items) if items.iter().all(JsonValue::is_string) => {
            CallToolResult::success(
                items
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(ContentBlock::text)
                    .collect(),
            )
        },
        other => {
            let mut result = CallToolResult::success(Vec::new());
            result.structured_content = Some(other.clone());
            result
        },
    }
}

/// A client hook's `PostToolUseFailure`: the error the host reported, as an
/// MCP error result.
#[must_use]
pub fn from_hook_failure(error: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.to_owned())])
}

/// A canonical gateway `tool_result` block, as parsed from any inbound wire.
#[must_use]
pub fn from_canonical_tool_result(
    content: &[CanonicalContent],
    structured_content: Option<&JsonValue>,
    meta: Option<&JsonValue>,
    is_error: bool,
) -> CallToolResult {
    let blocks = content.iter().filter_map(canonical_block).collect();
    let mut result = if is_error {
        CallToolResult::error(blocks)
    } else {
        CallToolResult::success(blocks)
    };
    result.structured_content = structured_content.cloned();
    result.meta = meta
        .and_then(JsonValue::as_object)
        .map(|map| MetaObject(map.clone()));
    result
}

fn canonical_block(content: &CanonicalContent) -> Option<ContentBlock> {
    match content {
        CanonicalContent::Text { text, .. } => Some(ContentBlock::text(text.clone())),
        CanonicalContent::Image {
            source: ImageSource::Base64 {
                media_type, data, ..
            },
            ..
        } => Some(ContentBlock::image(data.clone(), media_type.clone())),
        CanonicalContent::Image {
            source: ImageSource::Url { url, .. },
            ..
        } => Some(ContentBlock::text(url.clone())),
        CanonicalContent::ToolUse { .. }
        | CanonicalContent::ToolResult { .. }
        | CanonicalContent::Thinking { .. } => None,
    }
}
