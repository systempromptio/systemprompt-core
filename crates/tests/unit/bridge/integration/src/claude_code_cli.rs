//! Tests for the standalone Claude Code CLI marketplace writer: the schema
//! fields Claude Code requires (`owner`, `lastUpdated`), user-scoped install
//! entries, foreign-key preservation, the one-marketplace-per-manifest-
//! marketplace mapping, the ownership sidecar, and the safety rule that an
//! unparseable registry file is never silently clobbered.

use std::path::Path;

use serde_json::{Map, Value, json};
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManifestMarketplace, PluginEntry, SignedManifest,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{PluginId, Sha256Digest};
use systemprompt_bridge::integration::claude_code_cli::installed::installed_entry;
use systemprompt_bridge::integration::claude_code_cli::marketplace::{
    MarketplaceEntry, marketplace_value, strip_known_marketplace, upsert_known_marketplace,
};
use systemprompt_bridge::integration::claude_code_cli::{host_marketplaces, sidecar};
use systemprompt_bridge::integration::json_io::{object_entry, read_optional_object};
use systemprompt_identifiers::MarketplaceId;
use tempfile::tempdir;

fn names(ids: Vec<MarketplaceId>) -> Vec<String> {
    ids.iter().map(|id| id.as_str().to_owned()).collect()
}

fn ids(plugin_ids: &[PluginId]) -> Vec<&str> {
    plugin_ids.iter().map(PluginId::as_str).collect()
}

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
    let v = marketplace_value("acme", "Acme tooling", "v1", &entries, &[]);
    assert!(v["owner"].is_object(), "owner must be an object");
    assert_eq!(v["name"], json!("acme"), "name is the marketplace id");
    assert_eq!(v["description"], json!("Acme tooling"));
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

// A user's scalar at a key the bridge owns is a foreign shape, not a slot to
// overwrite: the write is refused and the value left as found.
#[test]
fn object_entry_refuses_a_non_object_slot_and_leaves_it_intact() {
    let mut root = Map::new();
    root.insert("enabledPlugins".to_owned(), json!("scalar"));
    let err = object_entry(&mut root, Path::new("settings.json"), "enabledPlugins")
        .expect_err("a scalar where an object is owned is a foreign shape");
    assert_eq!(err.key, "enabledPlugins");
    assert_eq!(err.found, "a string");
    assert_eq!(root["enabledPlugins"], json!("scalar"));

    root.insert("enabledPlugins".to_owned(), json!([]));
    let err = object_entry(&mut root, Path::new("settings.json"), "enabledPlugins")
        .expect_err("an array where an object is owned is a foreign shape");
    assert_eq!(err.found, "an array");
    assert_eq!(err.expected, "an object");
    assert_eq!(root["enabledPlugins"], json!([]));
}

#[test]
fn object_entry_creates_the_slot_when_absent() {
    let mut root = Map::new();
    let m = object_entry(&mut root, Path::new("settings.json"), "enabledPlugins").unwrap();
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
    upsert_known_marketplace(
        d.path(),
        &MarketplaceId::new("org-provisioned"),
        "2026-02-03T04:05:06Z",
    )
    .unwrap();
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
        br#"{"org-provisioned":{},"acme":{},"keep":{}}"#,
    )
    .unwrap();
    strip_known_marketplace(d.path(), &MarketplaceId::new("acme")).unwrap();
    let km = read(&d.path().join("known_marketplaces.json"));
    assert!(km.get("acme").is_none());
    assert!(
        km.get("org-provisioned").is_some(),
        "only the named marketplace is stripped"
    );
    assert!(km.get("keep").is_some(), "foreign marketplace preserved");
}

fn plugin(id: &str) -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new(id).unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files: vec![],
        hooks: Default::default(),
    }
}

fn manifest(plugins: Vec<PluginEntry>, marketplaces: Vec<ManifestMarketplace>) -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-09-05T00:00:00Z-deadbeef").unwrap(),
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-09-05T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        not_before: chrono::DateTime::parse_from_rfc3339("2026-09-05T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        user_id: systemprompt_identifiers::UserId::new("test-user"),
        tenant_id: None,
        user: None,
        plugins,
        skills: vec![],
        rules: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec![],
        host_model_protocols: Default::default(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        auto_update: Default::default(),
        diagnostics: Vec::new(),
        marketplaces,
    }
}

fn manifest_marketplace(id: &str, name: &str, plugin_ids: &[&str]) -> ManifestMarketplace {
    ManifestMarketplace {
        id: MarketplaceId::new(id),
        name: name.into(),
        plugin_ids: plugin_ids
            .iter()
            .map(|p| PluginId::try_new(*p).unwrap())
            .collect(),
        allow_cross_marketplace_dependencies_on: vec![],
        external_marketplaces: vec![],
    }
}

#[test]
fn a_manifest_that_lists_no_marketplaces_yields_no_host_marketplaces() {
    let m = manifest(vec![plugin("alpha"), plugin("beta")], vec![]);
    assert!(host_marketplaces(&m).is_empty());
}

#[test]
fn each_manifest_marketplace_becomes_one_host_marketplace_and_a_shared_plugin_is_in_both() {
    let m = manifest(
        vec![plugin("alpha"), plugin("beta")],
        vec![
            manifest_marketplace("core", "Core", &["alpha"]),
            manifest_marketplace("commerce", "Commerce", &["alpha", "beta"]),
        ],
    );
    let hosts = host_marketplaces(&m);
    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0].id.as_str(), "core");
    assert_eq!(hosts[0].name, "Core");
    assert_eq!(ids(&hosts[0].plugin_ids), vec!["alpha"]);
    assert_eq!(hosts[1].id.as_str(), "commerce");
    assert_eq!(ids(&hosts[1].plugin_ids), vec!["alpha", "beta"]);
}

#[test]
fn a_manifest_with_no_plugins_yields_no_host_marketplaces_even_if_it_names_some() {
    let m = manifest(vec![], vec![manifest_marketplace("core", "Core", &[])]);
    assert!(host_marketplaces(&m).is_empty());
}

#[test]
fn sidecar_round_trips_and_an_absent_sidecar_owns_nothing() {
    let d = tempdir().unwrap();
    assert!(
        sidecar::owned_marketplaces(d.path()).unwrap().is_empty(),
        "with no sidecar the bridge has no record of writing anything"
    );

    sidecar::write(
        d.path(),
        &sidecar::Owned {
            marketplaces: vec![MarketplaceId::new("core"), MarketplaceId::new("commerce")],
            dependency_keys: vec![],
            external_marketplaces: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        names(sidecar::owned_marketplaces(d.path()).unwrap()),
        vec!["core", "commerce"]
    );

    sidecar::remove(d.path()).unwrap();
    assert!(!d.path().join(sidecar::SIDECAR).exists());
    sidecar::remove(d.path()).expect("removing an absent sidecar is not an error");
}

#[test]
fn a_corrupt_sidecar_is_an_error_rather_than_an_empty_ownership_record() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join(sidecar::SIDECAR), b"{ not json").unwrap();

    let err = sidecar::owned_marketplaces(d.path())
        .expect_err("a present but unparseable sidecar must not read as absent");
    assert!(
        err.to_string().contains(sidecar::SIDECAR),
        "the error names the file the operator has to fix: {err}"
    );
}

#[test]
fn host_projection_removes_cowork_setup_without_changing_the_source_bundle() {
    use systemprompt_bridge::gateway::manifest::SkillEntry;
    use systemprompt_bridge::integration::claude_code_cli::filter_skills_for_host;
    let skills: Vec<SkillEntry> = [("setup_admin", vec!["cowork"]), ("admin_report", vec![])]
        .into_iter()
        .map(|(id, hosts)| {
            serde_json::from_value(serde_json::json!({
                "id":id,"name":id,"description":"test","file_path":"SKILL.md",
                "sha256":"0".repeat(64),"instructions":"test","hosts":hosts
            }))
            .expect("skill")
        })
        .collect();
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    for root in [source.path(), destination.path()] {
        for id in ["setup-admin", "admin-report", "legacy-research"] {
            let path = root.join("skills").join(id);
            std::fs::create_dir_all(&path).expect("directory");
            std::fs::write(path.join("SKILL.md"), "instructions").expect("skill file");
        }
    }
    filter_skills_for_host(destination.path(), &skills, "claude-code").expect("projection");
    assert!(!destination.path().join("skills/setup-admin").exists());
    assert!(
        destination
            .path()
            .join("skills/admin-report/SKILL.md")
            .exists()
    );
    assert!(
        destination
            .path()
            .join("skills/legacy-research/SKILL.md")
            .exists()
    );
    assert!(source.path().join("skills/setup-admin/SKILL.md").exists());
    filter_skills_for_host(destination.path(), &skills, "claude-code").expect("repeat");
    filter_skills_for_host(source.path(), &skills, "cowork").expect("cowork");
    assert!(source.path().join("skills/setup-admin/SKILL.md").exists());
}
