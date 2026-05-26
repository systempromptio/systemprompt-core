use systemprompt_models::mcp::capabilities::{
    MCP_APP_MIME_TYPE, McpAppsUiConfig, McpCspDomains, McpExtensionId, McpResourceUiMeta,
    ToolVisibility, default_visibility, model_only_visibility, visibility_to_json,
};

#[test]
fn mcp_extension_id_as_str_for_known_and_custom() {
    assert_eq!(McpExtensionId::McpAppsUi.as_str(), "io.modelcontextprotocol/ui");
    let c = McpExtensionId::custom("acme/foo");
    assert_eq!(c.as_str(), "acme/foo");
}

#[test]
fn mcp_extension_id_display_matches_as_str() {
    assert_eq!(
        McpExtensionId::McpAppsUi.to_string(),
        "io.modelcontextprotocol/ui"
    );
    assert_eq!(McpExtensionId::custom("x").to_string(), "x");
}

#[test]
fn mcp_apps_ui_config_default_includes_app_mime_type() {
    let c = McpAppsUiConfig::new();
    assert_eq!(c.mime_types, vec![MCP_APP_MIME_TYPE.to_owned()]);
    let j = c.to_json();
    assert_eq!(j["mimeTypes"][0], MCP_APP_MIME_TYPE);
}

#[test]
fn tool_visibility_default_is_model_and_display() {
    assert_eq!(ToolVisibility::default(), ToolVisibility::Model);
    assert_eq!(ToolVisibility::Model.to_string(), "model");
    assert_eq!(ToolVisibility::App.to_string(), "app");
}

#[test]
fn default_visibility_includes_model_and_app() {
    let v = default_visibility();
    assert_eq!(v.len(), 2);
    assert!(v.contains(&ToolVisibility::Model));
    assert!(v.contains(&ToolVisibility::App));
}

#[test]
fn model_only_visibility_excludes_app() {
    let v = model_only_visibility();
    assert_eq!(v, vec![ToolVisibility::Model]);
}

#[test]
fn visibility_to_json_serializes_array() {
    let j = visibility_to_json(&[ToolVisibility::Model, ToolVisibility::App]);
    let arr = j.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[test]
fn mcp_csp_domains_builder_chains_fields() {
    let d = McpCspDomains::builder()
        .connect_domain("https://a")
        .connect_domains(["https://b", "https://c"])
        .resource_domain("https://r")
        .resource_domains(["https://r2"])
        .frame_domain("https://f")
        .base_uri_domain("https://b")
        .build();
    assert_eq!(d.connect.len(), 3);
    assert_eq!(d.resources.len(), 2);
    assert_eq!(d.frames.len(), 1);
    assert_eq!(d.base_uri.len(), 1);
}

#[test]
fn mcp_csp_domains_empty_is_default() {
    let d = McpCspDomains::empty();
    assert!(d.connect.is_empty());
    assert!(d.resources.is_empty());
    assert!(d.frames.is_empty());
    assert!(d.base_uri.is_empty());
}

#[test]
fn mcp_resource_ui_meta_with_chain_then_to_json() {
    let csp = McpCspDomains::builder().connect_domain("https://x").build();
    let m = McpResourceUiMeta::new()
        .with_csp(csp)
        .with_prefers_border(true)
        .with_domain("example.com");
    let j = m.to_json();
    assert!(j.get("csp").is_some());
    assert_eq!(j["prefersBorder"], true);
    assert_eq!(j["domain"], "example.com");
}

#[test]
fn mcp_resource_ui_meta_to_meta_map_wraps_under_ui_key() {
    let m = McpResourceUiMeta::new().with_domain("x");
    let meta = m.to_meta_map();
    assert!(meta.contains_key("ui"));
}

#[test]
fn mcp_resource_ui_meta_with_csp_opt_overrides() {
    let m = McpResourceUiMeta::new().with_csp_opt(None);
    assert!(m.csp.is_none());
    let m = McpResourceUiMeta::new().with_csp_opt(Some(McpCspDomains::empty()));
    assert!(m.csp.is_some());
}

#[test]
fn mcp_resource_ui_meta_default_to_json_is_empty_object() {
    let j = McpResourceUiMeta::new().to_json();
    assert!(j.as_object().unwrap().is_empty());
}
