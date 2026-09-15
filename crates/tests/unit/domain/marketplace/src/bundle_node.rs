use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::PluginId;
use systemprompt_marketplace::bundle::{
    BundleContent, NODE_LOCKFILES, NODE_PACKAGE_FILE, build_plugin_bundle, node_lockfile,
};
use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_RELPATH, PluginManifest};
use systemprompt_models::services::{
    PluginAuthor, PluginComponentRef, PluginConfig, PluginDependency,
};
use tempfile::TempDir;

static NO_DISABLED: BTreeSet<String> = BTreeSet::new();

fn config(id: &str, dependencies: Vec<PluginDependency>) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: PluginAuthor {
            name: "test".to_owned(),
            email: "test@example.com".to_owned(),
        },
        keywords: vec![],
        license: "BSL-1.0".to_owned(),
        category: "demo".to_owned(),
        skills: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        rules: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        hooks: Default::default(),
        scripts: vec![],
        dependencies,
    }
}

fn plugin_root(files: &[(&str, &str)]) -> TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("app");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    root
}

fn build(root: &TempDir, config: &PluginConfig) -> BTreeMap<String, Vec<u8>> {
    let content = BundleContent {
        skills: &[],
        rules: &[],
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        artifacts: &[],
        plugins_root: root.path(),
        managed_files: &BTreeMap::new(),
    };
    build_plugin_bundle(config, &content)
        .expect("bundle")
        .into_iter()
        .map(|(path, file)| (path, file.bytes))
        .collect()
}

#[test]
fn bundle_ships_package_json_with_its_lockfile() {
    let root = plugin_root(&[
        (NODE_PACKAGE_FILE, r#"{"name":"app"}"#),
        ("package-lock.json", r#"{"lockfileVersion":3}"#),
    ]);
    let bundle = build(&root, &config("app", vec![]));
    assert_eq!(bundle[NODE_PACKAGE_FILE], br#"{"name":"app"}"#);
    assert_eq!(bundle["package-lock.json"], br#"{"lockfileVersion":3}"#);
}

#[test]
fn bundle_omits_node_files_without_a_supported_lockfile() {
    let root = plugin_root(&[
        (NODE_PACKAGE_FILE, r#"{"name":"app"}"#),
        ("yarn.lock", "# yarn"),
    ]);
    let bundle = build(&root, &config("app", vec![]));
    assert!(
        !bundle.contains_key(NODE_PACKAGE_FILE),
        "Claude Code would not install from yarn.lock, so shipping package.json alone is misleading"
    );
    assert!(!bundle.contains_key("yarn.lock"));
}

#[test]
fn lockfile_priority_matches_claude_code() {
    assert_eq!(
        NODE_LOCKFILES,
        [
            "bun.lock",
            "bun.lockb",
            "npm-shrinkwrap.json",
            "package-lock.json"
        ]
    );
    let root = plugin_root(&[
        (NODE_PACKAGE_FILE, "{}"),
        ("package-lock.json", "{}"),
        ("npm-shrinkwrap.json", "{}"),
    ]);
    assert_eq!(
        node_lockfile(&root.path().join("app")),
        Some("npm-shrinkwrap.json")
    );
    let bundle = build(&root, &config("app", vec![]));
    assert!(bundle.contains_key("npm-shrinkwrap.json"));
    assert!(
        !bundle.contains_key("package-lock.json"),
        "only the lockfile Claude Code would pick is shipped"
    );
}

#[test]
fn manifest_carries_dependencies_in_claude_code_shape() {
    let root = plugin_root(&[]);
    let config = config(
        "app",
        vec![
            PluginDependency {
                name: "audit-logger".to_owned(),
                marketplace: None,
                version: None,
            },
            PluginDependency {
                name: "b2c-cli".to_owned(),
                marketplace: Some("salesforce".to_owned()),
                version: Some("^2.0".to_owned()),
            },
        ],
    );
    let bundle = build(&root, &config);
    let raw: serde_json::Value = serde_json::from_slice(&bundle[PLUGIN_MANIFEST_RELPATH]).unwrap();
    assert_eq!(
        raw["dependencies"],
        serde_json::json!([
            "audit-logger",
            { "name": "b2c-cli", "version": "^2.0", "marketplace": "salesforce" }
        ])
    );
    let parsed: PluginManifest = serde_json::from_slice(&bundle[PLUGIN_MANIFEST_RELPATH]).unwrap();
    assert_eq!(parsed.dependencies.len(), 2);

    let without = build(&root, &config("app", vec![]));
    let raw: serde_json::Value = serde_json::from_slice(&without[PLUGIN_MANIFEST_RELPATH]).unwrap();
    assert!(
        raw.get("dependencies").is_none(),
        "an empty list is omitted so older hosts see the same manifest as before"
    );
}
