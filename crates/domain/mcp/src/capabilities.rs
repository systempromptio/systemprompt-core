//! MCP server capability declarations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use systemprompt_models::mcp::{
    LEGACY_RESOURCE_URI_META_KEY, McpAppsUiConfig, McpExtensionId, McpUiToolMeta, ToolVisibility,
    UI_META_KEY,
};

pub fn mcp_apps_ui_extension() -> (String, serde_json::Map<String, serde_json::Value>) {
    let config = McpAppsUiConfig::new();
    let key = McpExtensionId::McpAppsUi.as_str().to_owned();
    let mut value = serde_json::Map::new();
    value.insert("mimeTypes".to_owned(), serde_json::json!(config.mime_types));
    (key, value)
}

pub fn build_extension_capabilities() -> BTreeMap<String, serde_json::Map<String, serde_json::Value>>
{
    let mut map = BTreeMap::new();
    let (key, value) = mcp_apps_ui_extension();
    map.insert(key, value);
    map
}

pub fn default_tool_visibility() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model, ToolVisibility::App]
}

pub fn model_only_visibility() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model]
}

pub fn visibility_to_json(visibility: &[ToolVisibility]) -> serde_json::Value {
    serde_json::json!(visibility)
}

pub fn tool_ui_meta(
    server_name: &str,
    visibility: &[ToolVisibility],
) -> serde_json::Map<String, serde_json::Value> {
    let resource_uri = format!("ui://{server_name}/artifact-viewer");
    let ui_meta = McpUiToolMeta::new(resource_uri.clone()).with_visibility(visibility.to_vec());

    let mut meta = serde_json::Map::new();
    meta.insert(
        UI_META_KEY.to_owned(),
        serde_json::to_value(&ui_meta).unwrap_or(serde_json::Value::Null),
    );
    meta.insert(LEGACY_RESOURCE_URI_META_KEY.to_owned(), resource_uri.into());
    meta
}

pub const WEBSITE_URL: &str = "https://systemprompt.io";
