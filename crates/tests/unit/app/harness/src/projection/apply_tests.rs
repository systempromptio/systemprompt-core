use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use systemprompt_device::protocol::names::sha256_hex;
use systemprompt_harness::projection::{
    BundleFile, BundleFiles, Projection, ProjectionError, RULES_BEGIN,
};
use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::host::HostKind;

use crate::projection::fixtures::{self, projected, skill};

fn skills(ids: &[&str]) -> Projection {
    let list: Vec<Value> = ids.iter().map(|id| skill(id, json!({}))).collect();
    projected(
        &fixtures::manifest(json!({ "skills": list })),
        &BundleFiles::new(),
    )
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn writes_skills_and_the_bridge_ledger_and_rewrites_nothing_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let config = applier.config_dir().to_path_buf();
    let report = applier.apply(&skills(&["b_two", "a_one"])).unwrap();
    assert_eq!(report.counts.skills, 2);
    assert_eq!(report.sync_counts().skills, 2);
    assert_eq!(
        fixtures::tree(&config),
        [
            "skills/.systemprompt-managed.json",
            "skills/a-one/SKILL.md",
            "skills/b-two/SKILL.md"
        ]
    );
    assert_eq!(
        read(&config.join("skills/a-one/SKILL.md")),
        "---\nname: a-one\ndescription: Plain description.\n---\n\n# Body\n\nDo it.\n"
    );
    assert_eq!(
        read(&config.join("skills/.systemprompt-managed.json")),
        "{\n  \"version\": \"a55bcf1f4e8ae59a\",\n  \"ids\": [\n    \"a-one\",\n    \"b-two\"\n  ],\n  \"marketplaces\": [\n    \"alpha-local\",\n    \"astound-commons\"\n  ]\n}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let inode = |path: &str| fs::metadata(config.join(path)).unwrap().ino();
        let before = (
            inode("skills/a-one/SKILL.md"),
            inode("skills/.systemprompt-managed.json"),
        );
        applier.apply(&skills(&["a_one", "b_two"])).unwrap();
        let after = (
            inode("skills/a-one/SKILL.md"),
            inode("skills/.systemprompt-managed.json"),
        );
        assert_eq!(after, before);
    }
}

#[test]
fn a_skill_that_left_goes_with_its_directory_and_the_users_own_stay() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let config = applier.config_dir().to_path_buf();
    applier.apply(&skills(&["one", "two"])).unwrap();
    fs::create_dir_all(config.join("skills/mine")).unwrap();
    fs::write(
        config.join("skills/mine/SKILL.md"),
        "---\nname: mine\ndescription: Mine.\n---\n",
    )
    .unwrap();
    fs::create_dir_all(config.join("skills/one/references")).unwrap();
    fs::write(config.join("skills/one/references/notes.md"), "kept").unwrap();
    fs::create_dir_all(config.join("skills/two/references")).unwrap();
    fs::write(config.join("skills/two/references/notes.md"), "goes").unwrap();
    applier.apply(&skills(&["one"])).unwrap();
    assert_eq!(
        fixtures::tree(&config),
        [
            "skills/.systemprompt-managed.json",
            "skills/mine/SKILL.md",
            "skills/one/SKILL.md",
            "skills/one/references/notes.md"
        ]
    );
    applier
        .apply(&projected(
            &fixtures::manifest(json!({})),
            &BundleFiles::new(),
        ))
        .unwrap();
    assert_eq!(fixtures::tree(&config), ["skills/mine/SKILL.md"]);
}

#[test]
fn a_corrupt_ledger_fails_and_leaves_every_file() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let config = applier.config_dir().to_path_buf();
    applier.apply(&skills(&["one"])).unwrap();
    let ledger = config.join("skills/.systemprompt-managed.json");
    fs::write(&ledger, "{ not json").unwrap();
    assert!(matches!(
        applier.apply(&skills(&["two"])),
        Err(ProjectionError::LedgerJson { .. })
    ));
    assert!(matches!(
        applier.clear(),
        Err(ProjectionError::LedgerJson { .. })
    ));
    fs::write(&ledger, r#"{"version":"x","ids":["../escape"]}"#).unwrap();
    assert!(matches!(
        applier.clear(),
        Err(ProjectionError::LedgerEntry { .. })
    ));
    assert_eq!(
        fixtures::tree(&config),
        ["skills/.systemprompt-managed.json", "skills/one/SKILL.md"]
    );
}

#[test]
fn bundle_files_have_their_own_ledger_and_only_listed_files_go() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let config = applier.config_dir().to_path_buf();
    let theme = "{ \"base\": {} }\n";
    let theme_path = format!("hosts/{}/themes/astound.json", HostKind::Harness.as_str());
    let theme_sha = sha256_hex(theme.as_bytes());
    let manifest = fixtures::manifest(json!({
        "plugins": [{ "id": "commons", "files": [
            { "path": "agents/a.md", "sha256": sha256_hex(b"# A") },
            { "path": "agents/b.md", "sha256": sha256_hex(b"# B") },
            { "path": theme_path, "sha256": theme_sha }
        ] }],
        "marketplaces": [
            { "id": "m", "harness": { "themes": [{ "name": "astound", "sha": theme_sha }] } }
        ]
    }));
    let commons = PluginId::new("commons");
    let mut files = BundleFiles::new();
    files.insert(&commons, "agents/a.md", b"# A".to_vec());
    files.insert(&commons, "agents/b.md", b"# B".to_vec());
    files.insert(&commons, theme_path, theme.as_bytes().to_vec());
    let report = applier.apply(&projected(&manifest, &files)).unwrap();
    assert_eq!(report.counts.agents, 2);
    assert_eq!(
        read(&config.join(".systemprompt-owned.json")),
        "{\n  \"files\": [\n    \"agents/a.md\",\n    \"agents/b.md\",\n    \"themes/astound.json\"\n  ]\n}"
    );
    assert_eq!(
        read(&config.join("agents/a.md")),
        "---\nmode: subagent\n---\n\n# A"
    );
    assert_eq!(read(&config.join("themes/astound.json")), theme);
    fs::write(config.join("agents/mine.md"), "mine").unwrap();
    let mut fewer = BundleFiles::new();
    fewer.insert(&commons, "agents/b.md", b"# B".to_vec());
    fewer.insert(&commons, "agents/a.md", b"# tampered".to_vec());
    let report = applier.apply(&projected(&manifest, &fewer)).unwrap();
    assert_eq!(report.warnings.len(), 2);
    assert_eq!(
        fixtures::tree(&config),
        [".systemprompt-owned.json", "agents/b.md", "agents/mine.md"]
    );
    applier.clear().unwrap();
    assert_eq!(fixtures::tree(&config), ["agents/mine.md"]);
}

#[test]
fn an_unowned_bundle_path_is_refused_before_anything_is_written() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let mut projection = skills(&["one"]);
    projection.bundle_files.push(BundleFile {
        path: "../escape.md".to_owned(),
        bytes: b"x".to_vec(),
    });
    assert!(matches!(
        applier.apply(&projection),
        Err(ProjectionError::Unowned { .. })
    ));
    assert_eq!(fixtures::tree(applier.config_dir()), Vec::<String>::new());
}

#[test]
fn the_rules_block_follows_the_users_text_and_an_empty_file_is_deleted() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let agents = applier.config_dir().join("AGENTS.md");
    let with_rules = |instructions: &str| {
        let manifest = fixtures::manifest(json!({
            "rules": [{ "id": "r", "name": "Rule", "instructions": instructions, "hosts": [HostKind::Harness.as_str()] }]
        }));
        projected(&manifest, &BundleFiles::new())
    };
    let none = projected(&fixtures::manifest(json!({})), &BundleFiles::new());
    applier.apply(&none).unwrap();
    assert!(!agents.exists());
    applier.apply(&with_rules("first")).unwrap();
    let first = read(&agents);
    assert!(first.starts_with(RULES_BEGIN) && first.contains("## Rule\n\nfirst\n"));
    applier.clear().unwrap();
    assert!(!agents.exists());
    fs::write(&agents, "# Mine\n\nKeep this.\n").unwrap();
    applier.apply(&with_rules("first")).unwrap();
    applier.apply(&with_rules("second")).unwrap();
    let text = read(&agents);
    assert!(text.starts_with("# Mine\n\nKeep this.\n\n<!-- systemprompt:begin"));
    assert!(text.contains("second") && !text.contains("first"));
    applier.apply(&none).unwrap();
    assert_eq!(read(&agents), "# Mine\n\nKeep this.\n");
}
