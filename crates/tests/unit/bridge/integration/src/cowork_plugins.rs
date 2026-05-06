use systemprompt_bridge::integration::cowork_plugins::{
    InstalledPluginEntry, KnownMarketplaceEntry, LocalSource, MarketplaceFile, MarketplaceOwner,
    MarketplacePluginEntry, disable_plugin, enable_plugin, enabled_plugins_key, parse_root,
    parse_settings, render_marketplace, render_settings, upsert_installed_plugin,
    upsert_known_marketplace,
};

fn known(name: &str, path: &str) -> KnownMarketplaceEntry {
    KnownMarketplaceEntry {
        name: name.into(),
        source: LocalSource::local(path.into()),
        installed_at: Some("2026-05-06T00:00:00Z".into()),
    }
}

fn installed(marketplace: &str, name: &str) -> InstalledPluginEntry {
    InstalledPluginEntry {
        marketplace: marketplace.into(),
        name: name.into(),
        version: "1.0.0".into(),
        installed_at: Some("2026-05-06T00:00:00Z".into()),
    }
}

#[test]
fn enabled_plugins_key_is_plugin_at_marketplace() {
    assert_eq!(
        enabled_plugins_key("systemprompt-managed", "systemprompt-bridge-managed"),
        "systemprompt-managed@systemprompt-bridge-managed"
    );
}

#[test]
fn upsert_known_marketplace_inserts_then_replaces_then_unchanged() {
    let mut root = parse_root(b"").unwrap();

    let r1 = upsert_known_marketplace(&mut root, &known("mp", "/path/a")).unwrap();
    assert_eq!(r1.inserted, vec!["mp".to_string()]);

    let r2 = upsert_known_marketplace(&mut root, &known("mp", "/path/b")).unwrap();
    assert_eq!(r2.replaced, vec!["mp".to_string()]);

    let r3 = upsert_known_marketplace(&mut root, &known("mp", "/path/b")).unwrap();
    assert_eq!(r3.unchanged, vec!["mp".to_string()]);
}

#[test]
fn upsert_known_marketplace_preserves_foreign_entries_and_root_keys() {
    let raw = br#"{
        "marketplaces": [
            { "name": "user-mp", "source": { "type": "git", "url": "https://example.com" } }
        ],
        "anthropicAddedField": 42
    }"#;
    let mut root = parse_root(raw).unwrap();

    upsert_known_marketplace(&mut root, &known("systemprompt-bridge-managed", "/x")).unwrap();

    let bytes = serde_json::to_vec(&serde_json::Value::Object(root.clone())).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["anthropicAddedField"], 42);
    let marketplaces = parsed["marketplaces"].as_array().unwrap();
    assert_eq!(marketplaces.len(), 2);
    let names: Vec<&str> = marketplaces
        .iter()
        .filter_map(|v| v["name"].as_str())
        .collect();
    assert!(names.contains(&"user-mp"));
    assert!(names.contains(&"systemprompt-bridge-managed"));
}

#[test]
fn upsert_installed_plugin_keys_on_marketplace_plus_name() {
    let mut root = parse_root(b"").unwrap();

    let r1 = upsert_installed_plugin(&mut root, &installed("mp-a", "plugin-x")).unwrap();
    let r2 = upsert_installed_plugin(&mut root, &installed("mp-b", "plugin-x")).unwrap();
    assert_eq!(r1.inserted, vec!["mp-a::plugin-x".to_string()]);
    assert_eq!(r2.inserted, vec!["mp-b::plugin-x".to_string()]);

    let bytes = serde_json::to_vec(&serde_json::Value::Object(root)).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["installedPlugins"].as_array().unwrap().len(), 2);
}

#[test]
fn enable_plugin_adds_compound_key_preserving_others() {
    let raw = br#"{
        "enabledPlugins": { "user-plugin@user-mp": true },
        "theme": "dark"
    }"#;
    let mut root = parse_settings(raw).unwrap();

    let r = enable_plugin(&mut root, "systemprompt-managed", "systemprompt-bridge-managed").unwrap();
    assert!(r.set);
    assert!(!r.already);

    let bytes = render_settings(&root).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["theme"], "dark");
    let enabled = parsed["enabledPlugins"].as_object().unwrap();
    assert_eq!(enabled["user-plugin@user-mp"], true);
    assert_eq!(
        enabled["systemprompt-managed@systemprompt-bridge-managed"],
        true
    );
}

#[test]
fn enable_plugin_idempotent_on_already_true() {
    let mut root = parse_settings(b"").unwrap();
    enable_plugin(&mut root, "p", "m").unwrap();
    let r = enable_plugin(&mut root, "p", "m").unwrap();
    assert!(r.already);
    assert!(!r.set);
}

#[test]
fn disable_plugin_removes_only_its_own_key() {
    let raw = br#"{ "enabledPlugins": { "user-plugin@user-mp": true, "p@m": true } }"#;
    let mut root = parse_settings(raw).unwrap();
    let removed = disable_plugin(&mut root, "p", "m").unwrap();
    assert!(removed);

    let bytes = render_settings(&root).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let enabled = parsed["enabledPlugins"].as_object().unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled["user-plugin@user-mp"], true);
    assert!(!enabled.contains_key("p@m"));
}

#[test]
fn disable_plugin_when_no_enabled_plugins_key_is_noop() {
    let mut root = parse_settings(b"{}").unwrap();
    let removed = disable_plugin(&mut root, "p", "m").unwrap();
    assert!(!removed);
}

#[test]
fn render_marketplace_emits_local_source_shape() {
    let mp = MarketplaceFile {
        name: "systemprompt-bridge-managed".into(),
        owner: MarketplaceOwner {
            name: "systemprompt.io".into(),
            email: Some("support@systemprompt.io".into()),
        },
        plugins: vec![MarketplacePluginEntry {
            name: "systemprompt-managed".into(),
            source: LocalSource::local("./plugins/systemprompt-managed".into()),
            version: "1.0.0".into(),
            description: Some("Managed plugin from systemprompt.io".into()),
        }],
    };
    let bytes = render_marketplace(&mp).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["name"], "systemprompt-bridge-managed");
    let plugin = &parsed["plugins"][0];
    assert_eq!(plugin["name"], "systemprompt-managed");
    assert_eq!(plugin["source"]["type"], "local");
    assert_eq!(plugin["source"]["path"], "./plugins/systemprompt-managed");
    assert_eq!(plugin["version"], "1.0.0");
}
