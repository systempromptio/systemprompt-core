//! The `opencode` block on a manifest marketplace: tolerance of keys a newer
//! gateway adds, and the bridge floor it raises.

use serde_json::json;
use systemprompt_bridge::gateway::manifest::ManifestMarketplace;
use systemprompt_models::bridge::manifest::{
    OPENCODE_V2_MIN_BRIDGE, manifest_min_bridge_version, min_bridge_version,
};

fn parsed(opencode: serde_json::Value) -> ManifestMarketplace {
    serde_json::from_value(json!({
        "id": "acme",
        "name": "Acme",
        "plugin_ids": [],
        "opencode": opencode,
    }))
    .expect("parses")
}

#[test]
fn unknown_opencode_keys_from_a_newer_gateway_are_ignored() {
    let marketplace = parsed(json!({
        "min_client_version": "2.1.0",
        "theme": {"name": "acme", "sha": "a".repeat(64), "variant": "dark"},
        "cli_defaults": {"autoupdate": false},
        "future_setting": {"nested": true}
    }));
    let opencode = marketplace.opencode.expect("opencode block");
    assert_eq!(
        opencode.min_client_version,
        Some(semver::Version::new(2, 1, 0))
    );
    assert_eq!(opencode.default_theme.as_deref(), Some("acme"));
    assert_eq!(opencode.themes.len(), 1);
    assert_eq!(opencode.cli_defaults["autoupdate"], json!(false));
}

#[test]
fn an_opencode_block_raises_the_bridge_floor() {
    let without = parsed(json!(null));
    assert!(without.opencode.is_none());
    assert_eq!(
        manifest_min_bridge_version(std::slice::from_ref(&without)),
        min_bridge_version()
    );
    let with = parsed(json!({}));
    assert_eq!(manifest_min_bridge_version(&[with]), OPENCODE_V2_MIN_BRIDGE);
}

#[test]
fn a_single_theme_from_an_older_gateway_is_the_only_theme_and_the_default() {
    let opencode = parsed(json!({"theme": {"name": "acme", "sha": "a".repeat(64)}}))
        .opencode
        .expect("opencode block");
    assert_eq!(opencode.themes.len(), 1);
    assert_eq!(
        opencode.default_theme_ref().map(|t| t.name.as_str()),
        Some("acme")
    );
}

#[test]
fn several_themes_carry_a_default_and_a_mode() {
    let opencode = parsed(json!({
        "themes": [
            {"name": "acme", "sha": "a".repeat(64)},
            {"name": "acme-black", "sha": "b".repeat(64)}
        ],
        "default_theme": "acme-black",
        "theme_mode": "dark"
    }))
    .opencode
    .expect("opencode block");
    assert_eq!(opencode.themes.len(), 2);
    assert_eq!(
        opencode.default_theme_ref().map(|t| t.name.as_str()),
        Some("acme-black")
    );
    assert_eq!(
        opencode.theme_mode,
        Some(systemprompt_models::bridge::manifest::ThemeMode::Dark)
    );
}

#[test]
fn a_default_theme_that_names_no_declared_theme_selects_nothing() {
    let opencode = parsed(json!({
        "themes": [{"name": "acme", "sha": "a".repeat(64)}],
        "default_theme": "missing"
    }))
    .opencode
    .expect("opencode block");
    assert!(opencode.default_theme_ref().is_none());
}
