//! Tests for the standalone Claude Code CLI marketplace writer: the schema
//! fields Claude Code requires (`owner`, `lastUpdated`), user-scoped install
//! entries, foreign-key preservation, and the safety rule that an unparseable
//! registry file is never silently clobbered.

use std::path::Path;

use serde_json::{Map, Value, json};
use systemprompt_bridge::integration::claude_code_cli::json_io::{
    object_entry, read_optional_object,
};
use systemprompt_bridge::integration::claude_code_cli::marketplace::{
    MarketplaceEntry, installed_entry, marketplace_value, strip_known_marketplace,
    upsert_known_marketplace,
};
use tempfile::tempdir;

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn marketplace_value_has_required_owner_object() {
    // `claude plugin validate` fails with "owner: expected object" without this.
    let entries = vec![
        MarketplaceEntry {
            name: "plugin-a".into(),
            description: "Plugin A".into(),
            version: "1.0.0".into(),
        },
        MarketplaceEntry {
            name: "plugin-b".into(),
            description: "Plugin B".into(),
            version: "1.0.0".into(),
        },
    ];
    let v = marketplace_value("v1", &entries);
    assert!(v["owner"].is_object(), "owner must be an object");
    assert_eq!(v["name"], json!("org-provisioned"));
    assert_eq!(v["plugins"][0]["name"], json!("plugin-a"));
    assert_eq!(v["plugins"][0]["source"], json!("./plugins/plugin-a"));
    assert_eq!(v["plugins"][1]["name"], json!("plugin-b"));
}

#[test]
fn installed_entry_is_user_scoped_with_version() {
    let v = installed_entry(Path::new("/x/cache"), "v1", "2026-01-01T00:00:00Z");
    assert_eq!(v[0]["scope"], json!("user"));
    assert_eq!(v[0]["version"], json!("v1"));
    assert_eq!(v[0]["installedAt"], json!("2026-01-01T00:00:00Z"));
}

#[test]
fn read_optional_object_none_for_missing() {
    let d = tempdir().unwrap();
    assert!(
        read_optional_object(&d.path().join("nope.json"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn read_optional_object_strips_utf8_bom() {
    let d = tempdir().unwrap();
    let p = d.path().join("k.json");
    std::fs::write(&p, b"\xEF\xBB\xBF{\"a\":1}").unwrap();
    let m = read_optional_object(&p).unwrap().unwrap();
    assert_eq!(m["a"], json!(1));
}

#[test]
fn read_optional_object_aborts_on_malformed_without_clobbering() {
    // A file we can't parse (e.g. settings.json holding the user's token) must
    // surface an error, never be overwritten.
    let d = tempdir().unwrap();
    let p = d.path().join("k.json");
    std::fs::write(&p, b"{ not json").unwrap();
    assert!(read_optional_object(&p).is_err());
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"{ not json",
        "file left intact"
    );
}

#[test]
fn object_entry_coerces_non_object_slot() {
    let mut root = Map::new();
    root.insert("enabledPlugins".to_owned(), json!("scalar"));
    let m = object_entry(&mut root, "enabledPlugins").unwrap();
    m.insert("p@mp".to_owned(), Value::Bool(true));
    assert_eq!(root["enabledPlugins"]["p@mp"], Value::Bool(true));
}

#[test]
fn upsert_known_marketplace_writes_last_updated_and_preserves_foreign() {
    // `lastUpdated` is required ("expected string, received undefined") and a
    // user's own marketplaces must survive the upsert.
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("known_marketplaces.json"),
        br#"{"someones-mp":{"source":{"source":"github","repo":"a/b"}}}"#,
    )
    .unwrap();
    upsert_known_marketplace(d.path(), "2026-02-03T04:05:06Z").unwrap();
    let km = read(&d.path().join("known_marketplaces.json"));
    assert_eq!(
        km["org-provisioned"]["lastUpdated"],
        json!("2026-02-03T04:05:06Z")
    );
    assert_eq!(
        km["someones-mp"]["source"]["repo"],
        json!("a/b"),
        "foreign preserved"
    );
}

#[test]
fn strip_known_marketplace_removes_only_ours() {
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("known_marketplaces.json"),
        br#"{"org-provisioned":{},"keep":{}}"#,
    )
    .unwrap();
    strip_known_marketplace(d.path()).unwrap();
    let km = read(&d.path().join("known_marketplaces.json"));
    assert!(km.get("org-provisioned").is_none());
    assert!(km.get("keep").is_some(), "foreign marketplace preserved");
}
