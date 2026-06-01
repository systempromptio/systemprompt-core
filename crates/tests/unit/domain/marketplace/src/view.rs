use systemprompt_marketplace::{render_marketplace_json, render_marketplace_list};

use crate::helpers::marketplace;

#[test]
fn render_json_contains_id_and_version() {
    let mp = marketplace("acme");
    let json = render_marketplace_json("acme", &mp);

    assert_eq!(json["name"], "acme");
    assert_eq!(json["metadata"]["version"], "1.0.0");
}

#[test]
fn render_json_contains_author_name() {
    let mp = marketplace("acme");
    let json = render_marketplace_json("acme", &mp);

    assert_eq!(json["owner"]["name"], "test");
}

#[test]
fn render_json_plugins_empty_when_no_includes() {
    let mp = marketplace("acme");
    let json = render_marketplace_json("acme", &mp);

    assert!(
        json["plugins"]
            .as_array()
            .expect("plugins array")
            .is_empty()
    );
}

#[test]
fn render_json_plugins_lists_include_entries() {
    use crate::helpers::include;

    let mut mp = marketplace("acme");
    mp.plugins = include(&["plugin-a", "plugin-b"]);
    let json = render_marketplace_json("acme", &mp);

    let plugins = json["plugins"].as_array().expect("plugins array");
    assert_eq!(plugins.len(), 2);

    let names: Vec<&str> = plugins
        .iter()
        .map(|p| p["name"].as_str().expect("name field"))
        .collect();
    assert!(names.contains(&"plugin-a"));
    assert!(names.contains(&"plugin-b"));
}

#[test]
fn render_json_plugin_source_path() {
    use crate::helpers::include;

    let mut mp = marketplace("acme");
    mp.plugins = include(&["my-plugin"]);
    let json = render_marketplace_json("acme", &mp);

    let plugins = json["plugins"].as_array().expect("plugins array");
    let source = plugins[0]["source"].as_str().expect("source field");
    assert!(
        source.contains("my-plugin"),
        "source path must contain plugin id"
    );
}

#[test]
fn render_list_wraps_in_marketplaces_key() {
    use systemprompt_identifiers::MarketplaceId;

    let mp_a = marketplace("alpha");
    let mp_b = marketplace("beta");
    let list = [
        (MarketplaceId::new("alpha"), mp_a),
        (MarketplaceId::new("beta"), mp_b),
    ];
    let refs: Vec<_> = list.iter().map(|(id, mp)| (id, mp)).collect();
    let json = render_marketplace_list(refs);

    let entries = json["marketplaces"].as_array().expect("marketplaces array");
    assert_eq!(entries.len(), 2);
}

#[test]
fn render_list_contains_id_name_version() {
    use systemprompt_identifiers::MarketplaceId;

    let mp = marketplace("solo");
    let id = MarketplaceId::new("solo");
    let json = render_marketplace_list([(&id, &mp)]);

    let entry = &json["marketplaces"][0];
    assert_eq!(entry["id"], "solo");
    assert_eq!(entry["name"], "solo marketplace");
    assert_eq!(entry["version"], "1.0.0");
}

#[test]
fn render_list_empty_input_gives_empty_array() {
    let json = render_marketplace_list(std::iter::empty());

    let entries = json["marketplaces"].as_array().expect("marketplaces array");
    assert!(entries.is_empty());
}
