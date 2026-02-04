use std::collections::BTreeMap;
use systemprompt_models::mcp::{McpAppsUiConfig, McpExtensionId, ToolVisibility};

pub fn mcp_apps_ui_extension() -> (String, serde_json::Map<String, serde_json::Value>) {
    let config = McpAppsUiConfig::new();
    let key = McpExtensionId::McpAppsUi.as_str().to_string();
    let mut value = serde_json::Map::new();
    value.insert(
        "mimeTypes".to_string(),
        serde_json::json!(config.mime_types),
    );
    (key, value)
}

pub fn build_experimental_capabilities(
) -> BTreeMap<String, serde_json::Map<String, serde_json::Value>> {
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
    let mut meta = serde_json::Map::new();
    meta.insert(
        "ui".to_string(),
        serde_json::json!({
            "resourceUri": format!("ui://{server_name}/artifact-viewer"),
            "visibility": visibility
        }),
    );
    meta
}

pub const WEBSITE_URL: &str = "https://systemprompt.io";
