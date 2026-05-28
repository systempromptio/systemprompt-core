// `marketplace.json` is the **canonical anthropic shape** —
// `https://anthropic.com/claude-code/marketplace.schema.json`. It is *not* a
// host-specific snowflake. These tests pin the wire-level shape so any future
// drift in the renderer is caught before it reaches Cowork / Codex CLI / any
// other host that consumes the marketplace.

use serde_json::{Value, json};
use systemprompt_bridge::integration::cowork_plugins::{
    MARKETPLACE_SCHEMA_URL, MarketplaceFile, MarketplaceMetadata, MarketplaceOwner,
    MarketplacePluginEntry, render_marketplace,
};

fn sample() -> MarketplaceFile {
    MarketplaceFile {
        schema: Some(MARKETPLACE_SCHEMA_URL.into()),
        name: "systemprompt-bridge-managed".into(),
        description: Some("Skills and agents synced by the Systemprompt Bridge".into()),
        metadata: Some(MarketplaceMetadata {
            description: Some("Bridge-managed marketplace; contents come from org-plugins".into()),
            version: "1.0.0".into(),
            plugin_root: Some("./plugins".into()),
        }),
        owner: MarketplaceOwner {
            name: "systemprompt.io".into(),
            email: None,
        },
        plugins: vec![MarketplacePluginEntry {
            name: "systemprompt-managed".into(),
            source: "./plugins/systemprompt-managed".into(),
            version: "1.0.0".into(),
            description: Some("Org-managed plugin".into()),
            author: None,
            category: None,
        }],
    }
}

fn render(file: &MarketplaceFile) -> Value {
    serde_json::from_slice(&render_marketplace(file).expect("render")).expect("parse rendered")
}

#[test]
fn schema_url_is_anthropic_canonical() {
    assert_eq!(
        MARKETPLACE_SCHEMA_URL,
        "https://anthropic.com/claude-code/marketplace.schema.json"
    );
}

#[test]
fn rendered_has_dollar_schema_top_level() {
    let v = render(&sample());
    assert_eq!(v["$schema"], MARKETPLACE_SCHEMA_URL);
}

#[test]
fn plugin_source_is_plain_string_not_object() {
    // Critical: Cowork (and any canonical reader) treats `plugins[].source` as
    // a relative-path string, NOT an object with `{type, path}`. Earlier
    // versions of the bridge wrote the object form, which broke Cowork's
    // marketplace loader.
    let v = render(&sample());
    let src = &v["plugins"][0]["source"];
    assert!(src.is_string(), "plugins[].source must be a string, got: {src}");
    assert_eq!(src.as_str().unwrap(), "./plugins/systemprompt-managed");
}

#[test]
fn metadata_block_is_optional_but_well_formed_when_present() {
    let v = render(&sample());
    let md = &v["metadata"];
    assert_eq!(md["version"], "1.0.0");
    assert_eq!(md["pluginRoot"], "./plugins");
    assert!(md["description"].is_string());
}

#[test]
fn metadata_omitted_when_none() {
    let mut f = sample();
    f.metadata = None;
    f.schema = None;
    f.description = None;
    let v = render(&f);
    assert!(v.get("metadata").is_none(), "metadata key must be absent");
    assert!(v.get("$schema").is_none(), "$schema key must be absent");
    assert!(v.get("description").is_none(), "description key must be absent");
}

#[test]
fn round_trip_preserves_shape() {
    let original = sample();
    let json_bytes = render_marketplace(&original).unwrap();
    let parsed: MarketplaceFile = serde_json::from_slice(&json_bytes).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn negative_object_source_does_not_deserialize_as_plugin_entry() {
    // Belt-and-braces: if the wire ever flips back to the legacy object-source
    // shape, MarketplacePluginEntry will refuse to deserialize.
    let bad = json!({
        "name": "x",
        "source": { "type": "local", "path": "./y" },
        "version": "1.0.0"
    });
    let r: Result<MarketplacePluginEntry, _> = serde_json::from_value(bad);
    assert!(r.is_err(), "expected deserialize error for object-source");
}
