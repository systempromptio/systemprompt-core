//! The Claude Code CLI emitter's handling of plugin dependencies: the
//! cross-marketplace allowlist in `marketplace.json`, the foreign
//! `enabledPlugins` / `extraKnownMarketplaces` entries, and the sidecar
//! record that lets a later run take exactly those back.

use std::path::Path;

use serde_json::{Map, Value, json};
use systemprompt_bridge::integration::claude_code_cli::foreign::{
    ForeignRefs, apply_settings, collect,
};
use systemprompt_bridge::integration::claude_code_cli::marketplace::{
    HostMarketplace, marketplace_value,
};
use systemprompt_bridge::integration::claude_code_cli::sidecar;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::manifest::{ExternalMarketplace, ExternalMarketplaceSource};
use tempfile::tempdir;

fn salesforce() -> ExternalMarketplace {
    ExternalMarketplace {
        name: "salesforce".into(),
        source: ExternalMarketplaceSource::Github {
            repo: "SalesforceCommerceCloud/claude-plugins".into(),
        },
    }
}

fn host_marketplace(id: &str, external: Vec<ExternalMarketplace>) -> HostMarketplace {
    HostMarketplace {
        id: MarketplaceId::new(id),
        name: id.into(),
        plugin_ids: vec![],
        allow_cross_marketplace_dependencies_on: external.iter().map(|m| m.name.clone()).collect(),
        external_marketplaces: external,
    }
}

fn write_plugin(root: &Path, id: &str, dependencies: Value) {
    let dir = root.join(id).join(".claude-plugin");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.json"),
        json!({ "name": id, "dependencies": dependencies }).to_string(),
    )
    .unwrap();
}

#[test]
fn marketplace_json_carries_the_cross_marketplace_allowlist_only_when_set() {
    let without = marketplace_value("org", "Org", "v1", &[], &[]);
    assert!(without.get("allowCrossMarketplaceDependenciesOn").is_none());

    let with = marketplace_value("org", "Org", "v1", &[], &["salesforce".to_owned()]);
    assert_eq!(
        with["allowCrossMarketplaceDependenciesOn"],
        json!(["salesforce"])
    );
}

#[test]
fn collect_keys_only_dependencies_that_leave_the_mirrored_marketplaces() {
    let root = tempdir().unwrap();
    write_plugin(
        root.path(),
        "app",
        json!([
            "helper",
            { "name": "shared", "marketplace": "platform" },
            { "name": "b2c-cli", "marketplace": "salesforce", "version": "^2" },
            { "name": "b2c", "marketplace": "salesforce" }
        ]),
    );
    write_plugin(root.path(), "helper", json!([]));
    let marketplace = host_marketplace("org", vec![salesforce()]);
    let mirrored = [MarketplaceId::new("org"), MarketplaceId::new("platform")];
    let dirs = [root.path().join("app"), root.path().join("helper")];
    let dirs: Vec<&Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();

    let refs = collect(&marketplace, &dirs, &mirrored);

    assert_eq!(
        refs.dependency_keys.iter().cloned().collect::<Vec<_>>(),
        vec!["b2c-cli@salesforce", "b2c@salesforce"],
        "same-marketplace and sibling-marketplace dependencies are already enabled by the mirror"
    );
    assert_eq!(refs.external_marketplaces, vec![salesforce()]);
}

#[test]
fn collect_tolerates_a_plugin_without_a_manifest() {
    let root = tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("bare")).unwrap();
    let refs = collect(
        &host_marketplace("org", vec![]),
        &[&root.path().join("bare")],
        &[MarketplaceId::new("org")],
    );
    assert_eq!(refs, ForeignRefs::default());
}

fn settings_with(enabled: &[&str], known: &[&str]) -> Map<String, Value> {
    let mut root = Map::new();
    root.insert(
        "enabledPlugins".into(),
        Value::Object(
            enabled
                .iter()
                .map(|k| ((*k).to_owned(), json!(true)))
                .collect(),
        ),
    );
    root.insert(
        "extraKnownMarketplaces".into(),
        Value::Object(
            known
                .iter()
                .map(|k| {
                    (
                        (*k).to_owned(),
                        json!({ "source": { "source": "git", "url": "x" } }),
                    )
                })
                .collect(),
        ),
    );
    root
}

#[test]
fn apply_settings_writes_foreign_entries_and_removes_only_previously_owned_ones() {
    let mut root = settings_with(
        &["mine@user-added", "old-dep@vendor"],
        &["user-market", "vendor"],
    );
    let previous = sidecar::Owned {
        marketplaces: vec![MarketplaceId::new("org")],
        dependency_keys: vec!["old-dep@vendor".into()],
        external_marketplaces: vec!["vendor".into()],
    };
    let mut current = ForeignRefs::default();
    current.dependency_keys.insert("b2c-cli@salesforce".into());
    current.external_marketplaces.push(salesforce());

    apply_settings(&mut root, Path::new("settings.json"), &previous, &current).unwrap();

    let enabled = root["enabledPlugins"].as_object().unwrap();
    assert_eq!(enabled["mine@user-added"], json!(true), "user keys survive");
    assert!(
        enabled.get("old-dep@vendor").is_none(),
        "our stale key is removed"
    );
    assert_eq!(enabled["b2c-cli@salesforce"], json!(true));

    let known = root["extraKnownMarketplaces"].as_object().unwrap();
    assert!(
        known.contains_key("user-market"),
        "user marketplaces survive"
    );
    assert!(
        known.get("vendor").is_none(),
        "our stale marketplace is removed"
    );
    assert_eq!(
        known["salesforce"],
        json!({ "source": { "source": "github", "repo": "SalesforceCommerceCloud/claude-plugins" } })
    );
}

#[test]
fn apply_settings_with_nothing_current_takes_back_everything_recorded() {
    let mut root = settings_with(&["b2c@salesforce", "keep@other"], &["salesforce", "other"]);
    let previous = sidecar::Owned {
        marketplaces: vec![],
        dependency_keys: vec!["b2c@salesforce".into()],
        external_marketplaces: vec!["salesforce".into()],
    };
    apply_settings(
        &mut root,
        Path::new("settings.json"),
        &previous,
        &ForeignRefs::default(),
    )
    .unwrap();
    assert_eq!(root["enabledPlugins"], json!({ "keep@other": true }));
    assert_eq!(
        root["extraKnownMarketplaces"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["other"]
    );
}

#[test]
fn sidecar_round_trips_foreign_ownership_and_reads_the_old_shape() {
    let d = tempdir().unwrap();
    let owned = sidecar::Owned {
        marketplaces: vec![MarketplaceId::new("org")],
        dependency_keys: vec!["b2c@salesforce".into()],
        external_marketplaces: vec!["salesforce".into()],
    };
    sidecar::write(d.path(), &owned).unwrap();
    assert_eq!(sidecar::read(d.path()).unwrap(), owned);

    std::fs::write(
        d.path().join(sidecar::SIDECAR),
        r#"{"marketplaces":["legacy"]}"#,
    )
    .unwrap();
    let legacy = sidecar::read(d.path()).unwrap();
    assert_eq!(legacy.marketplaces, vec![MarketplaceId::new("legacy")]);
    assert!(legacy.dependency_keys.is_empty());
    assert!(legacy.external_marketplaces.is_empty());
}
