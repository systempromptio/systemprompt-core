use serde_json::json;
use systemprompt_device::protocol::names::sha256_hex;
use systemprompt_harness::projection::{
    BundleFiles, Counts, DefaultTheme, Projection, wanted_bundle_files,
};
use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::ThemeMode;

use crate::projection::fixtures::{self, projected, skill};

fn dirs(projection: &Projection) -> Vec<String> {
    projection
        .skills
        .as_ref()
        .map(|set| set.files.iter().map(|file| file.dir.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn a_host_that_is_not_enabled_projects_nothing() {
    let files = BundleFiles::new();
    for enabled in [json!(["claude-code"]), json!([])] {
        let manifest = fixtures::manifest(
            json!({ "enabled_hosts": enabled, "skills": [skill("kept", json!({}))] }),
        );
        let projection = projected(&manifest, &files);
        assert!(!projection.enabled);
        assert!(projection.skills.is_none());
        assert_eq!(projection.counts, Counts::default());
        assert_eq!(
            wanted_bundle_files(&manifest, HostKind::Harness),
            Vec::<(
                PluginId,
                String,
                systemprompt_models::bridge::ids::Sha256Digest
            )>::new()
        );
    }
}

#[test]
fn skills_need_the_host_a_body_and_a_plain_id() {
    let manifest = fixtures::manifest(json!({ "skills": [
        skill("kept", json!({})),
        skill("other", json!({ "hosts": ["claude-code"] })),
        skill("named", json!({ "hosts": ["claude-code", HostKind::Harness.as_str()] })),
        skill("bare", json!({ "instructions": " " })),
        skill("-lead", json!({})),
    ] }));
    let projection = projected(&manifest, &BundleFiles::new());
    assert_eq!(dirs(&projection), ["kept", "named"]);
    assert_eq!(projection.counts.skills, 2);
    assert_eq!(projection.warnings.len(), 1);
    assert_eq!(
        projection
            .skills
            .as_ref()
            .map(|set| set.marketplaces.clone()),
        Some(vec!["alpha-local".to_owned(), "astound-commons".to_owned()])
    );
}

#[test]
fn no_skills_at_all_removes_everything_but_unselected_skills_keep_an_empty_ledger() {
    let none = fixtures::manifest(json!({}));
    assert!(projected(&none, &BundleFiles::new()).skills.is_none());
    let elsewhere = fixtures::manifest(
        json!({ "skills": [skill("other", json!({ "hosts": ["claude-code"] }))] }),
    );
    let projection = projected(&elsewhere, &BundleFiles::new());
    assert_eq!(projection.skills.map(|set| set.files.len()), Some(0));
}

#[test]
fn a_shared_directory_or_an_unnameable_id_keeps_the_first() {
    let manifest = fixtures::manifest(json!({ "skills": [
        skill("a_b", json!({})),
        skill("a-b", json!({})),
        skill("a.b", json!({}))
    ] }));
    let projection = projected(&manifest, &BundleFiles::new());
    assert_eq!(dirs(&projection), ["a-b"]);
    assert_eq!(
        projection
            .skills
            .as_ref()
            .map(|set| set.files[0].id.as_str()),
        Some("a_b")
    );
    assert_eq!(projection.warnings.len(), 2);
}

#[test]
fn owners_publications_governance_and_rules() {
    let manifest = fixtures::manifest(json!({
        "skills": [
            skill("alpha_notes", json!({
                "plugins": ["alpha-tools"],
                "publication": {
                    "bundle_digest": sha256_hex(b"bundle"),
                    "generation": 1,
                    "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
                    "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
                    "revision_id": "802c1429-27ed-4274-afd9-51e2ed79ea79"
                }
            })),
            skill("who_am_i", json!({
                "source": { "plugin": "commons", "path": "skills/who-am-i/SKILL.md" },
                "plugins": ["x"]
            })),
            skill("loose", json!({})),
        ],
        "plugins": [
            { "id": "alpha-tools", "hooks": { "governance": false } },
            { "id": "commons", "hooks": { "governance": true, "judge": true } }
        ],
        "rules": [
            { "id": "conventions", "name": "Conventions", "instructions": "Be kind.", "hosts": [HostKind::Harness.as_str()] }
        ]
    }));
    let projection = projected(&manifest, &BundleFiles::new());
    assert_eq!(
        projection.governance_plugin.as_ref().map(PluginId::as_str),
        Some("commons")
    );
    let owners: Vec<(&str, &str)> = projection
        .skill_owners
        .iter()
        .map(|(s, p)| (s.as_str(), p.as_str()))
        .collect();
    assert_eq!(
        owners,
        [("alpha_notes", "alpha-tools"), ("who_am_i", "commons")]
    );
    assert_eq!(projection.publications.len(), 1);
    assert_eq!(projection.publications[0].dir, "alpha-notes");
    assert_eq!(
        projection.skill_dir(&projection.publications[0].skill),
        Some("alpha-notes")
    );
    assert_eq!(projection.counts.rules, 1);
    assert!(
        projection
            .rules_block
            .as_deref()
            .is_some_and(|block| block.contains("## Conventions\n\nBe kind.\n"))
    );
    let applied = projection.applied();
    assert_eq!(applied.counts, projection.counts);
    assert_eq!(applied.instance_key.as_str(), "alpha");
}

#[test]
fn defaults_take_the_first_marketplace_and_the_theme_needs_its_file() {
    let theme = "{ \"base\": {} }\n";
    let theme_path = format!("hosts/{}/themes/astound.json", HostKind::Harness.as_str());
    let sha = sha256_hex(theme.as_bytes());
    let manifest = fixtures::manifest(json!({
        "plugins": [{ "id": "commons", "files": [{ "path": theme_path, "sha256": sha }] }],
        "marketplaces": [
            { "id": "first", "harness": {
                "defaults": { "model": "systemprompt/claude-sonnet-5-5", "share": "disabled" }
            } },
            { "id": "second", "harness": {
                "defaults": { "model": "systemprompt/other", "small_model": "s" },
                "themes": [{ "name": "astound", "sha": sha }],
                "default_theme": "astound",
                "theme_mode": "dark"
            } }
        ]
    }));
    let without = projected(&manifest, &BundleFiles::new());
    assert_eq!(
        serde_json::Value::Object(without.defaults.clone()),
        json!({ "model": "claude-sonnet-5-5", "share": "disabled", "small_model": "s" })
    );
    assert_eq!(without.default_theme, None);
    assert_eq!(without.warnings.len(), 1);
    let mut files = BundleFiles::new();
    files.insert(
        &PluginId::new("commons"),
        theme_path,
        theme.as_bytes().to_vec(),
    );
    let with = projected(&manifest, &files);
    assert_eq!(
        with.default_theme,
        Some(DefaultTheme {
            name: "astound".to_owned(),
            mode: Some(ThemeMode::Dark)
        })
    );
    assert_eq!(with.bundle_files.len(), 1);
    assert_eq!(wanted_bundle_files(&manifest, HostKind::Harness).len(), 1);
}

#[test]
fn plan_is_total_on_odd_entries() {
    let manifest = fixtures::manifest(json!({
        "skills": [
            skill("_", json!({})),
            skill("x", json!({ "frontmatter": { "k": { "deep": [1, { "z": null }] } } }))
        ],
        "managed_mcp_servers": [{ "name": "", "url": "not a url" }],
        "plugins": [{ "id": ".hidden", "files": [{ "path": "agents/a.md", "sha256": sha256_hex(b"a") }] }],
        "rules": [{ "id": "r", "name": "", "hosts": [HostKind::Harness.as_str()] }]
    }));
    let projection = projected(&manifest, &BundleFiles::new());
    assert!(projection.enabled);
    assert_eq!(dirs(&projection), ["x"]);
    assert_eq!(
        projection.mcp,
        Vec::<systemprompt_harness::projection::ManagedMcp>::new()
    );
    assert_eq!(
        projection.bundle_files,
        Vec::<systemprompt_harness::projection::BundleFile>::new()
    );
    assert_eq!(projection.counts.rules, 1);
}
