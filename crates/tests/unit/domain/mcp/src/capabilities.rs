//! Unit tests for capabilities module (re-exports at crate root).

use systemprompt_mcp::{
    WEBSITE_URL, build_extension_capabilities, default_tool_visibility, mcp_apps_ui_extension,
    model_only_visibility, tool_ui_meta, visibility_to_json,
};

#[test]
fn test_website_url_constant() {
    assert_eq!(WEBSITE_URL, "https://systemprompt.io");
}

#[test]
fn test_mcp_apps_ui_extension_key_and_value_shape() {
    let (key, value) = mcp_apps_ui_extension();
    assert!(!key.is_empty());
    assert!(value.contains_key("mimeTypes"));
    let mime_types = value.get("mimeTypes").expect("mimeTypes present");
    assert!(mime_types.is_array() || mime_types.is_string() || mime_types.is_object());
}

#[test]
fn test_build_extension_capabilities_contains_apps_ui_key() {
    let caps = build_extension_capabilities();
    let (expected_key, _) = mcp_apps_ui_extension();
    assert!(caps.contains_key(&expected_key));
    assert_eq!(caps.len(), 1);
}

#[test]
fn test_default_tool_visibility_two_entries() {
    let vis = default_tool_visibility();
    assert_eq!(vis.len(), 2);
}

#[test]
fn test_model_only_visibility_single_entry() {
    let vis = model_only_visibility();
    assert_eq!(vis.len(), 1);
}

#[test]
fn test_default_and_model_only_are_different() {
    assert_ne!(default_tool_visibility(), model_only_visibility());
}

#[test]
fn test_visibility_to_json_default_array() {
    let json = visibility_to_json(&default_tool_visibility());
    assert!(json.is_array());
    assert_eq!(json.as_array().expect("array").len(), 2);
}

#[test]
fn test_visibility_to_json_empty() {
    let json = visibility_to_json(&[]);
    assert!(json.is_array());
    assert_eq!(json.as_array().expect("array").len(), 0);
}

#[test]
fn test_tool_ui_meta_contains_ui_key() {
    let meta = tool_ui_meta("my-server", &default_tool_visibility());
    assert!(meta.contains_key("ui"));
}

#[test]
fn test_tool_ui_meta_resource_uri_includes_server_name() {
    let meta = tool_ui_meta("my-server", &default_tool_visibility());
    let ui = meta.get("ui").expect("ui");
    let uri = ui
        .get("resourceUri")
        .and_then(|v| v.as_str())
        .expect("resourceUri str");
    assert!(uri.contains("my-server"));
    assert!(uri.starts_with("ui://"));
    assert!(uri.ends_with("/artifact-viewer"));
}

#[test]
fn test_tool_ui_meta_visibility_serializes() {
    let meta = tool_ui_meta("srv", &model_only_visibility());
    let ui = meta.get("ui").expect("ui");
    let vis = ui.get("visibility").expect("visibility");
    assert!(vis.is_array());
    assert_eq!(vis.as_array().expect("array").len(), 1);
}

#[test]
fn test_tool_ui_meta_empty_server_name() {
    let meta = tool_ui_meta("", &default_tool_visibility());
    let uri = meta
        .get("ui")
        .and_then(|u| u.get("resourceUri"))
        .and_then(|v| v.as_str())
        .expect("uri");
    assert_eq!(uri, "ui:///artifact-viewer");
}
