// `known_marketplaces.json` is the **Cowork adapter** layer — Cowork's
// per-session install-state file, NOT a marketplace schema. These tests pin
// the shape Cowork's `NativeMarketplaceReader` expects (keyed top-level
// object). Foreign sibling entries (other marketplaces the user installed
// themselves) MUST be preserved through upsert.

use serde_json::{Value, json};
use systemprompt_bridge::integration::cowork_plugins::{
    KnownMarketplaceEntry, KnownMarketplacesFile, LocalSource, parse_root,
    retain_known_marketplaces, upsert_known_marketplace,
};

fn entry(name: &str, path: &str) -> KnownMarketplaceEntry {
    KnownMarketplaceEntry {
        name: name.into(),
        source: LocalSource::local(path.into()),
        install_location: path.into(),
        last_updated: "2026-05-28T12:00:00Z".into(),
    }
}

#[test]
fn upsert_writes_keyed_object_shape() {
    let mut root = serde_json::Map::new();
    upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/x")).unwrap();
    // The marketplace **name** is the top-level JSON key, not an `array[].name`.
    assert!(root.contains_key("bridge-managed"));
    let v = &root["bridge-managed"];
    assert_eq!(v["source"]["source"], "local");
    assert_eq!(v["source"]["path"], "C:/mp/x");
    assert_eq!(v["installLocation"], "C:/mp/x");
    assert_eq!(v["lastUpdated"], "2026-05-28T12:00:00Z");
}

#[test]
fn typed_file_round_trip_matches_wire() {
    let mut root = serde_json::Map::new();
    upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/x")).unwrap();
    let json = serde_json::to_string(&Value::Object(root.clone())).unwrap();
    let parsed: KnownMarketplacesFile = serde_json::from_str(&json).unwrap();
    assert!(parsed.contains("bridge-managed"));
}

#[test]
fn foreign_marketplace_preserved_through_upsert() {
    let mut root = serde_json::Map::new();
    // User added their own marketplace via the Cowork UI before we ran.
    root.insert(
        "user-favourite-mp".into(),
        json!({
            "source": { "source": "local", "path": "C:/somewhere/else" },
            "installLocation": "C:/somewhere/else",
            "lastUpdated": "2026-01-01T00:00:00Z"
        }),
    );
    upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/x")).unwrap();
    assert!(root.contains_key("user-favourite-mp"), "foreign entry vanished");
    assert!(root.contains_key("bridge-managed"));
}

#[test]
fn upsert_replaces_existing_idempotently() {
    let mut root = serde_json::Map::new();
    let r1 = upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/x")).unwrap();
    assert_eq!(r1.inserted, vec!["bridge-managed"]);
    let r2 = upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/x")).unwrap();
    assert_eq!(r2.unchanged, vec!["bridge-managed"]);
    let r3 = upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/y")).unwrap();
    assert_eq!(r3.replaced, vec!["bridge-managed"]);
}

#[test]
fn retain_removes_only_our_key() {
    let mut root = serde_json::Map::new();
    upsert_known_marketplace(&mut root, &entry("bridge-managed", "C:/mp/x")).unwrap();
    root.insert("user-favourite-mp".into(), json!({}));
    retain_known_marketplaces(&mut root, "bridge-managed");
    assert!(!root.contains_key("bridge-managed"));
    assert!(root.contains_key("user-favourite-mp"));
}

#[test]
fn parse_root_handles_empty_file() {
    // First-time install: file doesn't exist yet, upsert gets an empty buffer.
    let root = parse_root(b"").unwrap();
    assert!(root.is_empty());
}
