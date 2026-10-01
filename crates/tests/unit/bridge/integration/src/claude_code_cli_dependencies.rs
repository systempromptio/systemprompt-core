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
    HostMarketplace, append_external_plugins, marketplace_value,
};
use systemprompt_bridge::integration::claude_code_cli::sidecar;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::manifest::{
    ManifestExternalMarketplace, ManifestExternalMarketplaceSource, ManifestExternalPlugin,
    ManifestExternalPluginSource, ManifestMarketplace,
};
use tempfile::tempdir;

const SHA: &str = "74354ecc7a43da16d91a9bc54fa8db8283a3fcf5";

fn salesforce() -> ManifestExternalMarketplace {
    ManifestExternalMarketplace {
        name: "salesforce".into(),
        source: ManifestExternalMarketplaceSource {
            source: "github".into(),
            repo: Some("SalesforceCommerceCloud/claude-plugins".into()),
            ..Default::default()
        },
    }
}

fn playwright() -> ManifestExternalPlugin {
    ManifestExternalPlugin {
        name: "playwright-cli".into(),
        source: ManifestExternalPluginSource {
            source: "git-subdir".into(),
            url: Some("microsoft/playwright-cli".into()),
            path: Some("skills".into()),
            reference: Some("v0.1.21".into()),
            sha: Some(SHA.into()),
            ..Default::default()
        },
        description: None,
        version: Some("0.1.21".into()),
        strict: Some(false),
        skills: Some(systemprompt_models::services::ExternalPluginSkills::Paths(
            vec!["./".into()],
        )),
    }
}

fn host_marketplace(id: &str, external: Vec<ManifestExternalMarketplace>) -> HostMarketplace {
    HostMarketplace {
        id: MarketplaceId::new(id),
        name: id.into(),
        plugin_ids: vec![],
        allow_cross_marketplace_dependencies_on: external.iter().map(|m| m.name.clone()).collect(),
        external_marketplaces: external,
        external_plugins: vec![],
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

#[test]
fn the_catalog_carries_each_pass_through_entry_as_authored() {
    let mut catalog = marketplace_value("org", "Org", "v1", &[], &[]);
    append_external_plugins(&mut catalog, &[playwright()]).unwrap();
    assert_eq!(
        catalog["plugins"],
        json!([{
            "name": "playwright-cli",
            "source": {
                "source": "git-subdir",
                "url": "microsoft/playwright-cli",
                "path": "skills",
                "ref": "v0.1.21",
                "sha": SHA
            },
            "version": "0.1.21",
            "strict": false,
            "skills": ["./"]
        }])
    );
}

#[test]
fn collect_enables_a_bare_name_dependency_on_a_pass_through_plugin() {
    let root = tempdir().unwrap();
    write_plugin(root.path(), "app", json!(["playwright-cli", "helper"]));
    let mut marketplace = host_marketplace("org", vec![]);
    marketplace.external_plugins = vec![playwright()];
    let refs = collect(
        &marketplace,
        &[&root.path().join("app")],
        &[MarketplaceId::new("org")],
    );
    assert_eq!(
        refs.dependency_keys.iter().cloned().collect::<Vec<_>>(),
        vec!["playwright-cli@org"],
        "the mirror never enables a pass-through plugin, so its dependency key is collected"
    );
}

#[test]
fn extra_known_marketplaces_carries_the_ref_pin() {
    let mut pinned = salesforce();
    pinned.source.reference = Some("b2c-agent-plugins@1.10.0".into());
    let mut current = ForeignRefs::default();
    current.external_marketplaces.push(pinned);
    let mut root = settings_with(&[], &[]);
    apply_settings(
        &mut root,
        Path::new("settings.json"),
        &sidecar::Owned::default(),
        &current,
    )
    .unwrap();
    assert_eq!(
        root["extraKnownMarketplaces"]["salesforce"],
        json!({ "source": {
            "source": "github",
            "repo": "SalesforceCommerceCloud/claude-plugins",
            "ref": "b2c-agent-plugins@1.10.0"
        } })
    );
}

#[test]
fn a_manifest_marketplace_with_ref_and_external_plugins_parses_and_so_does_one_without() {
    let newer: ManifestMarketplace = serde_json::from_value(json!({
        "id": "org",
        "name": "Org",
        "plugin_ids": ["app"],
        "external_marketplaces": [{
            "name": "salesforce",
            "source": {
                "source": "github",
                "repo": "SalesforceCommerceCloud/claude-plugins",
                "ref": "b2c-agent-plugins@1.10.0",
                "future_key": true
            },
            "future_key": 1
        }],
        "external_plugins": [{
            "name": "playwright-cli",
            "source": {
                "source": "git-subdir",
                "url": "microsoft/playwright-cli",
                "path": "skills",
                "ref": "v0.1.21",
                "sha": SHA
            },
            "strict": false,
            "skills": ["./"],
            "version": "0.1.21",
            "future_key": "ignored"
        }],
        "future_key": "ignored"
    }))
    .expect("a newer gateway's marketplace parses");
    assert_eq!(
        newer.external_marketplaces[0].source.reference.as_deref(),
        Some("b2c-agent-plugins@1.10.0")
    );
    assert_eq!(newer.external_plugins, vec![playwright()]);

    let older: ManifestMarketplace = serde_json::from_value(json!({
        "id": "org",
        "name": "Org",
        "plugin_ids": ["app"],
        "external_marketplaces": [{
            "name": "salesforce",
            "source": { "source": "github", "repo": "SalesforceCommerceCloud/claude-plugins" }
        }]
    }))
    .expect("an older gateway's marketplace parses");
    assert_eq!(older.external_marketplaces, vec![salesforce()]);
    assert!(older.external_plugins.is_empty());
}
