//! The `ui://<server>/document-preview` MCP App resource that renders a
//! `documents_export` preview, built the same way as the artifact
//! viewer resource.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::{
    CacheScope, ListResourcesResult, MetaObject, ReadResourceRequestParams, ReadResourceResult,
    Resource, ResourceContents,
};
use systemprompt_identifiers::McpServerId;
use systemprompt_models::mcp::{
    MCP_APP_MIME_TYPE, McpUiToolMeta, UI_META_KEY, ui_method_js_constants,
};

pub const PREVIEW_RESOURCE_NAME: &str = "document-preview";
const TEMPLATE_TTL_MS: u64 = 3_600_000;
const CONSTANTS_PLACEHOLDER: &str = "/*MCP_UI_CONSTANTS*/";

#[must_use]
pub fn preview_resource_uri(server_name: &McpServerId) -> String {
    format!("ui://{server_name}/{PREVIEW_RESOURCE_NAME}")
}

#[must_use]
pub fn preview_template() -> String {
    include_str!("document-preview.html").replace(CONSTANTS_PLACEHOLDER, &ui_method_js_constants())
}

#[must_use]
pub fn preview_resource(server_name: &McpServerId) -> Resource {
    let template = preview_template();
    Resource::new(preview_resource_uri(server_name), "Document Preview")
        .with_title("Document preview".to_owned())
        .with_description("Renders the HTML preview of a document revision".to_owned())
        .with_mime_type(MCP_APP_MIME_TYPE.to_owned())
        .with_size(u64::try_from(template.len()).unwrap_or(u64::MAX))
}

#[must_use]
pub fn preview_resource_list(server_name: &McpServerId) -> ListResourcesResult {
    ListResourcesResult::with_all_items(vec![preview_resource(server_name)])
        .with_ttl_ms(TEMPLATE_TTL_MS)
        .with_cache_scope(CacheScope::Public)
}

pub fn read_preview_resource(
    request: &ReadResourceRequestParams,
    server_name: &McpServerId,
) -> Result<ReadResourceResult, McpError> {
    let expected = preview_resource_uri(server_name);
    if request.uri != expected {
        return Err(McpError::invalid_params(
            format!(
                "Unknown resource URI: {}. Expected: {expected}",
                request.uri
            ),
            None,
        ));
    }
    let contents = ResourceContents::TextResourceContents {
        uri: expected,
        mime_type: Some(MCP_APP_MIME_TYPE.to_owned()),
        text: preview_template(),
        meta: None,
    };
    Ok(ReadResourceResult::new(vec![contents])
        .with_ttl_ms(TEMPLATE_TTL_MS)
        .with_cache_scope(CacheScope::Public))
}

// JSON: MCP tool `_meta` — the spec types it as an open object.
#[must_use]
pub fn preview_tool_meta(server_name: &McpServerId) -> MetaObject {
    let ui = McpUiToolMeta::new(preview_resource_uri(server_name));
    let mut meta = serde_json::Map::new();
    meta.insert(
        UI_META_KEY.to_owned(),
        serde_json::to_value(&ui).unwrap_or(serde_json::Value::Null),
    );
    MetaObject(meta)
}
