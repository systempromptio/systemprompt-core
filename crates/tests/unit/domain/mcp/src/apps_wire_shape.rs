// Pins the MCP Apps extension wire shapes to the normative schema at
// https://github.com/modelcontextprotocol/ext-apps. A host silently ignores
// anything it does not recognise, so a rename here fails loudly instead.

use systemprompt_models::mcp::{
    EXTENSION_ID, LATEST_PROTOCOL_VERSION, LEGACY_RESOURCE_URI_META_KEY, McpCspDomains,
    McpUiToolMeta, RESOURCE_MIME_TYPE, SizeChangedParams, ToolVisibility, UI_META_KEY,
    UiInitializeParams, UiMessageParams, UiMethod, ui_method_js_constants,
};

#[test]
fn extension_id_matches_spec() {
    assert_eq!(EXTENSION_ID, "io.modelcontextprotocol/ui");
    assert_eq!(RESOURCE_MIME_TYPE, "text/html;profile=mcp-app");
    assert_eq!(LATEST_PROTOCOL_VERSION, "2026-01-26");
    assert_eq!(UI_META_KEY, "ui");
    assert_eq!(LEGACY_RESOURCE_URI_META_KEY, "ui/resourceUri");
}

#[test]
fn ui_method_strings_match_spec() {
    assert_eq!(UiMethod::Initialize.as_str(), "ui/initialize");
    assert_eq!(
        UiMethod::Initialized.as_str(),
        "ui/notifications/initialized"
    );
    assert_eq!(
        UiMethod::ToolResult.as_str(),
        "ui/notifications/tool-result"
    );
    assert_eq!(
        UiMethod::SizeChanged.as_str(),
        "ui/notifications/size-changed"
    );
    assert_eq!(UiMethod::Message.as_str(), "ui/message");
}

#[test]
fn ui_method_serde_matches_as_str() {
    for method in UiMethod::all() {
        let json = serde_json::to_string(method).expect("serialize");
        assert_eq!(json, format!("\"{}\"", method.as_str()));
    }
}

#[test]
fn tool_meta_serializes_resource_uri_and_visibility() {
    let meta = McpUiToolMeta::new("ui://weather/view.html")
        .with_visibility(vec![ToolVisibility::Model, ToolVisibility::App]);
    let json = serde_json::to_value(&meta).expect("serialize");

    assert_eq!(json["resourceUri"], "ui://weather/view.html");
    assert_eq!(json["visibility"][0], "model");
    assert_eq!(json["visibility"][1], "app");
    assert!(json.get("csp").is_none());
    assert!(json.get("permissions").is_none());
}

#[test]
fn csp_domains_use_the_schema_field_names() {
    let csp = McpCspDomains::builder()
        .connect_domain("https://api.example.com")
        .resource_domain("https://cdn.example.com")
        .frame_domain("https://frame.example.com")
        .base_uri_domain("https://example.com")
        .build();
    let json = serde_json::to_value(&csp).expect("serialize");

    assert_eq!(json["connectDomains"][0], "https://api.example.com");
    assert_eq!(json["resourceDomains"][0], "https://cdn.example.com");
    assert_eq!(json["frameDomains"][0], "https://frame.example.com");
    assert_eq!(json["baseUriDomains"][0], "https://example.com");
}

#[test]
fn size_changed_params_carry_width_and_height() {
    let params = SizeChangedParams {
        width: Some(800.0),
        height: Some(600.0),
    };
    let json = serde_json::to_value(params).expect("serialize");

    assert_eq!(json["width"], 800.0);
    assert_eq!(json["height"], 600.0);
}

#[test]
fn ui_message_content_is_an_array_of_blocks() {
    let params = UiMessageParams::user_text("Please analyze this data");
    let json = serde_json::to_value(&params).expect("serialize");

    assert_eq!(json["role"], "user");
    let content = json["content"].as_array().expect("content is an array");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Please analyze this data");
}

#[test]
fn ui_initialize_params_carry_all_three_required_fields() {
    let params = UiInitializeParams::new("TicTacToe", "1.0.0");
    let json = serde_json::to_value(&params).expect("serialize");

    assert_eq!(json["appInfo"]["name"], "TicTacToe");
    assert_eq!(json["appInfo"]["version"], "1.0.0");
    assert!(json.get("appCapabilities").is_some());
    assert_eq!(json["protocolVersion"], LATEST_PROTOCOL_VERSION);
}

#[test]
fn generated_js_constants_cover_every_method() {
    let js = ui_method_js_constants();
    for method in UiMethod::all() {
        assert!(
            js.contains(&format!("{}: '{}'", method.js_const(), method.as_str())),
            "missing JS constant for {method}"
        );
    }
    assert!(js.contains(LATEST_PROTOCOL_VERSION));
}
