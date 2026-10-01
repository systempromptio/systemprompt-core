//! MCP server capability declarations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::mcp::{
    McpAppsUiConfig, McpExtensionId, McpUiToolMeta, ToolVisibility, UI_META_KEY,
};

// JSON: MCP capability `extensions` entry — the spec types each as an open
// object.
pub fn mcp_apps_ui_extension() -> (String, serde_json::Map<String, serde_json::Value>) {
    let config = McpAppsUiConfig::new();
    let key = McpExtensionId::McpAppsUi.as_str().to_owned();
    let mut value = serde_json::Map::new();
    value.insert("mimeTypes".to_owned(), serde_json::json!(config.mime_types));
    (key, value)
}

// JSON: MCP capability `extensions` — the spec types each as an open object.
pub fn build_extension_capabilities() -> BTreeMap<String, serde_json::Map<String, serde_json::Value>>
{
    let mut map = BTreeMap::new();
    let (key, value) = mcp_apps_ui_extension();
    map.insert(key, value);
    map.insert(
        McpExtensionId::Tasks.as_str().to_owned(),
        serde_json::Map::new(),
    );
    map
}

pub fn default_tool_visibility() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model, ToolVisibility::App]
}

pub fn model_only_visibility() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model]
}

// JSON: MCP Apps tool `_meta.ui.visibility` — emitted into the open `_meta`
// object.
pub fn visibility_to_json(visibility: &[ToolVisibility]) -> serde_json::Value {
    serde_json::json!(visibility)
}

// JSON: MCP tool `_meta` — the spec types it as an open object.
pub fn tool_ui_meta(
    server_name: &McpServerId,
    visibility: &[ToolVisibility],
) -> serde_json::Map<String, serde_json::Value> {
    let resource_uri = format!("ui://{server_name}/artifact-viewer");
    let ui_meta = McpUiToolMeta::new(resource_uri).with_visibility(visibility.to_vec());

    let mut meta = serde_json::Map::new();
    meta.insert(
        UI_META_KEY.to_owned(),
        serde_json::to_value(&ui_meta).unwrap_or(serde_json::Value::Null),
    );
    meta
}

pub const WEBSITE_URL: &str = "https://systemprompt.io";
