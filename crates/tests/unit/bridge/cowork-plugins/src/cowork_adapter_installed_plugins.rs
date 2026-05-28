// `installed_plugins.json` shape. This is the file whose **previous wrong
// shape** (top-level `{ installedPlugins: [...] }`) made Cowork's
// `LocalPluginsReader.getAllLocalPluginsWithResolver` crash with
// `Cannot convert undefined or null to object` at `Object.entries(plugins)`.
// The tests below pin every property of the shape Cowork actually iterates so
// that regression cannot return silently.

use serde_json::{Value, json};
use systemprompt_bridge::integration::cowork_plugins::{
    InstalledPluginEntry, InstalledPluginsFile, installed_plugin_key, retain_installed_plugin,
    upsert_installed_plugin,
};

fn entry(name: &str, marketplace: &str, install_path: &str) -> InstalledPluginEntry {
    InstalledPluginEntry {
        marketplace: marketplace.into(),
        name: name.into(),
        scope: "user".into(),
        install_path: install_path.into(),
        version: "1.0.0".into(),
        installed_at: "2026-05-28T12:00:00Z".into(),
        last_updated: "2026-05-28T12:00:00Z".into(),
    }
}

#[test]
fn version_envelope_is_set_to_2() {
    let mut root = serde_json::Map::new();
    upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    assert_eq!(root["version"], 2);
}

#[test]
fn plugins_is_an_object_keyed_by_plugin_at_marketplace() {
    let mut root = serde_json::Map::new();
    upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    // Cowork's reader does Object.entries(plugins) — so `plugins` must be an
    // object, not undefined and not an array.
    let plugins = root["plugins"]
        .as_object()
        .expect("`plugins` must be a JSON object");
    assert!(plugins.contains_key("p@mp"));
}

#[test]
fn each_plugin_value_is_an_array_of_installs() {
    let mut root = serde_json::Map::new();
    upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    let installs = root["plugins"]["p@mp"]
        .as_array()
        .expect("value at <plugin>@<mp> must be an array");
    assert_eq!(installs.len(), 1);
    let install = &installs[0];
    assert_eq!(install["scope"], "user");
    assert_eq!(install["installPath"], "C:/x");
    assert_eq!(install["version"], "1.0.0");
    assert!(install["installedAt"].is_string());
    assert!(install["lastUpdated"].is_string());
}

#[test]
fn installed_plugin_key_uses_plugin_at_marketplace_order() {
    let e = entry("systemprompt-managed", "systemprompt-bridge-managed", "C:/x");
    // The exact key form Cowork's reader splits on `@`. Reversing the order
    // would silently break the join with `cowork_settings.json::enabledPlugins`.
    assert_eq!(
        installed_plugin_key(&e),
        "systemprompt-managed@systemprompt-bridge-managed"
    );
}

#[test]
fn typed_file_round_trip() {
    let mut root = serde_json::Map::new();
    upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    let s = serde_json::to_string(&Value::Object(root)).unwrap();
    let parsed: InstalledPluginsFile = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed.version, 2);
    assert!(parsed.plugins.contains_key("p@mp"));
}

#[test]
fn foreign_plugins_preserved() {
    let mut root = serde_json::Map::new();
    root.insert("version".into(), json!(2));
    root.insert(
        "plugins".into(),
        json!({
            "other@user-mp": [{
                "scope": "user",
                "installPath": "C:/elsewhere",
                "version": "0.1.0",
                "installedAt": "2026-01-01T00:00:00Z",
                "lastUpdated": "2026-01-01T00:00:00Z"
            }]
        }),
    );
    upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    let plugins = root["plugins"].as_object().unwrap();
    assert!(plugins.contains_key("other@user-mp"), "foreign plugin lost");
    assert!(plugins.contains_key("p@mp"));
}

#[test]
fn upsert_is_idempotent() {
    let mut root = serde_json::Map::new();
    let r1 = upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    assert_eq!(r1.inserted, vec!["p@mp"]);
    let r2 = upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    assert_eq!(r2.unchanged, vec!["p@mp"]);
}

#[test]
fn retain_removes_only_target_key() {
    let mut root = serde_json::Map::new();
    upsert_installed_plugin(&mut root, &entry("p", "mp", "C:/x")).unwrap();
    upsert_installed_plugin(&mut root, &entry("q", "user-mp", "C:/y")).unwrap();
    retain_installed_plugin(&mut root, "p@mp");
    let plugins = root["plugins"].as_object().unwrap();
    assert!(!plugins.contains_key("p@mp"));
    assert!(plugins.contains_key("q@user-mp"));
}
