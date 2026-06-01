use systemprompt_mcp::{
    WEBSITE_URL, build_experimental_capabilities, default_tool_visibility, mcp_apps_ui_extension,
    model_only_visibility, tool_ui_meta, visibility_to_json,
};

#[test]
fn website_url_is_https() {
    assert!(WEBSITE_URL.starts_with("https://"));
}

#[test]
fn website_url_contains_systemprompt() {
    assert!(WEBSITE_URL.contains("systemprompt"));
}

#[test]
fn mcp_apps_ui_extension_key_nonempty() {
    let (key, _) = mcp_apps_ui_extension();
    assert!(!key.is_empty());
}

#[test]
fn mcp_apps_ui_extension_mime_types_is_array_or_scalar() {
    let (_, value) = mcp_apps_ui_extension();
    let mime = value.get("mimeTypes").expect("mimeTypes");
    assert!(mime.is_array() || mime.is_string() || mime.is_object());
}

#[test]
fn build_experimental_capabilities_is_btree_sorted() {
    let caps = build_experimental_capabilities();
    let keys: Vec<_> = caps.keys().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "BTreeMap should be sorted");
}

#[test]
fn visibility_to_json_model_only_has_one_entry() {
    let json = visibility_to_json(&model_only_visibility());
    assert_eq!(json.as_array().expect("array").len(), 1);
}

#[test]
fn tool_ui_meta_visibility_array_length_matches_input() {
    let vis = default_tool_visibility();
    let meta = tool_ui_meta("srv", &vis);
    let ui = meta.get("ui").expect("ui");
    let arr = ui
        .get("visibility")
        .and_then(|v| v.as_array())
        .expect("array");
    assert_eq!(arr.len(), vis.len());
}

#[test]
fn tool_ui_meta_model_only_visibility() {
    let meta = tool_ui_meta("srv", &model_only_visibility());
    let arr = meta
        .get("ui")
        .and_then(|u| u.get("visibility"))
        .and_then(|v| v.as_array())
        .expect("array");
    assert_eq!(arr.len(), 1);
}

#[test]
fn tool_ui_meta_resource_uri_scheme() {
    let meta = tool_ui_meta("server-123", &default_tool_visibility());
    let uri = meta
        .get("ui")
        .and_then(|u| u.get("resourceUri"))
        .and_then(|v| v.as_str())
        .expect("uri");
    assert!(uri.starts_with("ui://"));
}

#[test]
fn tool_ui_meta_resource_uri_path_component() {
    let meta = tool_ui_meta("abc-server", &[]);
    let uri = meta
        .get("ui")
        .and_then(|u| u.get("resourceUri"))
        .and_then(|v| v.as_str())
        .expect("uri");
    assert!(uri.contains("artifact-viewer"));
}

#[test]
fn default_tool_visibility_and_model_only_differ_in_len() {
    assert!(default_tool_visibility().len() > model_only_visibility().len());
}

#[test]
fn visibility_to_json_serializable() {
    let json = visibility_to_json(&default_tool_visibility());
    let _s = serde_json::to_string(&json).expect("serialize");
}
