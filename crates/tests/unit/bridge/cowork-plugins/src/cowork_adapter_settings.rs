// `cowork_settings.json::enabledPlugins` — keyed-object map of
// `"<plugin>@<marketplace>" -> bool`. Same Cowork-internal contract as
// installed_plugins.json; foreign keys (user's other plugins) must survive
// our upsert.

use systemprompt_bridge::integration::cowork_plugins::{
    disable_plugin, enable_plugin, enabled_plugins_key, parse_settings, render_settings,
};

#[test]
fn key_uses_plugin_at_marketplace_order() {
    assert_eq!(
        enabled_plugins_key("systemprompt-managed", "org-provisioned"),
        "systemprompt-managed@org-provisioned"
    );
}

#[test]
fn enable_writes_keyed_object_value_true() {
    let mut root = serde_json::Map::new();
    let report = enable_plugin(&mut root, "p", "mp").unwrap();
    assert!(report.set);
    assert_eq!(root["enabledPlugins"]["p@mp"], serde_json::json!(true));
}

#[test]
fn enable_is_idempotent() {
    let mut root = serde_json::Map::new();
    enable_plugin(&mut root, "p", "mp").unwrap();
    let report = enable_plugin(&mut root, "p", "mp").unwrap();
    assert!(report.already);
    assert!(!report.set);
}

#[test]
fn disable_removes_only_target_key() {
    let mut root = serde_json::Map::new();
    enable_plugin(&mut root, "p", "mp").unwrap();
    enable_plugin(&mut root, "q", "user-mp").unwrap();
    let removed = disable_plugin(&mut root, "p", "mp").unwrap();
    assert!(removed);
    let enabled = root["enabledPlugins"].as_object().unwrap();
    assert!(!enabled.contains_key("p@mp"));
    assert!(enabled.contains_key("q@user-mp"));
}

#[test]
fn parse_settings_tolerates_utf8_bom() {
    // A UTF-8 BOM-prefixed file (e.g. written by PowerShell `Set-Content -Encoding utf8`)
    // must still parse, not fail with "expected value at line 1 column 1".
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(br#"{"enabledPlugins":{"p@mp":true}}"#);
    let parsed = parse_settings(&bytes).unwrap();
    assert_eq!(parsed["enabledPlugins"]["p@mp"], serde_json::json!(true));
}

#[test]
fn render_then_parse_round_trip() {
    let mut root = serde_json::Map::new();
    enable_plugin(&mut root, "p", "mp").unwrap();
    let bytes = render_settings(&root).unwrap();
    let parsed = parse_settings(&bytes).unwrap();
    assert_eq!(parsed["enabledPlugins"]["p@mp"], serde_json::json!(true));
}

#[test]
fn foreign_top_level_keys_preserved() {
    let mut root = serde_json::Map::new();
    // The Cowork app may write other top-level keys in cowork_settings.json
    // — `theme`, `lastOpenedAt`, etc. We must not clobber them on upsert.
    root.insert("theme".into(), serde_json::json!("dark"));
    enable_plugin(&mut root, "p", "mp").unwrap();
    assert_eq!(root["theme"], serde_json::json!("dark"));
}
