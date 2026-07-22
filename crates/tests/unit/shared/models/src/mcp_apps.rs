use systemprompt_models::mcp::apps::{
    LATEST_PROTOCOL_VERSION, McpUiToolMeta, UiInitializeParams, UiMessageParams, UiMethod,
    ui_method_js_constants,
};

#[test]
fn ui_method_serde_rename_matches_as_str_for_every_variant() {
    for method in UiMethod::all() {
        let json = serde_json::to_string(method).unwrap();
        assert_eq!(json, format!("\"{}\"", method.as_str()));
        let back: UiMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(back, *method);
    }
}

#[test]
fn ui_method_all_is_exhaustive_and_distinct() {
    let strs: std::collections::HashSet<&str> =
        UiMethod::all().iter().map(|m| m.as_str()).collect();
    assert_eq!(strs.len(), UiMethod::all().len());
    let consts: std::collections::HashSet<&str> =
        UiMethod::all().iter().map(|m| m.js_const()).collect();
    assert_eq!(consts.len(), UiMethod::all().len());
    assert!(strs.iter().all(|s| s.starts_with("ui/")));
}

#[test]
fn ui_method_display_matches_wire_name() {
    assert_eq!(
        UiMethod::ToolInputPartial.to_string(),
        "ui/notifications/tool-input-partial"
    );
}

#[test]
fn js_constants_projection_contains_every_method_and_version() {
    let js = ui_method_js_constants();
    assert!(js.starts_with("const MCP_UI = Object.freeze({"));
    for method in UiMethod::all() {
        assert!(js.contains(&format!("{}: '{}'", method.js_const(), method.as_str())));
    }
    assert!(js.contains(&format!("PROTOCOL_VERSION: '{LATEST_PROTOCOL_VERSION}'")));
}

#[test]
fn tool_meta_serializes_camel_case_and_omits_unset_fields() {
    let meta = McpUiToolMeta::new("ui://app/main");
    let value = serde_json::to_value(&meta).unwrap();
    assert_eq!(value["resourceUri"], "ui://app/main");
    assert!(value.get("visibility").is_none());
    assert!(value.get("csp").is_none());
    assert!(value.get("permissions").is_none());
}

#[test]
fn ui_message_content_is_an_array_of_blocks() {
    let params = UiMessageParams::user_text("hello");
    let value = serde_json::to_value(&params).unwrap();
    assert_eq!(value["role"], "user");
    assert!(value["content"].is_array());
    assert_eq!(value["content"][0]["type"], "text");
    assert_eq!(value["content"][0]["text"], "hello");
}

#[test]
fn ui_initialize_params_pin_latest_protocol_version() {
    let params = UiInitializeParams::new("my-app", "1.2.3");
    let value = serde_json::to_value(&params).unwrap();
    assert_eq!(value["protocolVersion"], LATEST_PROTOCOL_VERSION);
    assert_eq!(value["appInfo"]["name"], "my-app");
    assert_eq!(value["appInfo"]["version"], "1.2.3");
    assert_eq!(value["appCapabilities"], serde_json::json!({}));
}
