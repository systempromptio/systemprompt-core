use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use sp_projection::{
    Applier, BundleFiles, OWNED_LEDGER, ProjectionError, SKILLS_DIR, SKILLS_LEDGER,
};

use crate::fixtures::{self, skill};

fn write_skills_ledger(applier: &Applier, text: &str) {
    let dir = applier.config_dir().join(SKILLS_DIR);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(SKILLS_LEDGER), text).unwrap();
}

fn write_owned_ledger(applier: &Applier, text: &str) {
    fs::create_dir_all(applier.config_dir()).unwrap();
    fs::write(applier.config_dir().join(OWNED_LEDGER), text).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn the_skills_ledger_is_pretty_printed_without_a_final_newline() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let manifest = fixtures::manifest(json!({
        "skills": [skill("a_one", json!({}))],
        "marketplaces": [{ "id": "alpha-local" }]
    }));
    applier
        .apply(&fixtures::projected(&manifest, &BundleFiles::new()))
        .unwrap();
    let text = read(&applier.config_dir().join(SKILLS_DIR).join(SKILLS_LEDGER));
    let parsed: Value = serde_json::from_str(&text).unwrap();
    let version = parsed["version"].as_str().unwrap_or_default().to_owned();
    assert_eq!(version.len(), 16);
    assert!(version.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        text,
        format!(
            "{{\n  \"version\": \"{version}\",\n  \"ids\": [\n    \"a-one\"\n  ],\n  \"marketplaces\": [\n    \"alpha-local\"\n  ]\n}}"
        )
    );
}

#[test]
fn corrupt_or_foreign_ledgers_fail() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    assert!(applier.clear().is_ok());
    write_skills_ledger(&applier, "{ not json");
    assert!(matches!(
        applier.clear(),
        Err(ProjectionError::LedgerJson { .. })
    ));
    write_skills_ledger(&applier, r#"{"version":"x","ids":["../escape"]}"#);
    assert!(matches!(
        applier.clear(),
        Err(ProjectionError::LedgerEntry { .. })
    ));
    fs::remove_file(applier.config_dir().join(SKILLS_DIR).join(SKILLS_LEDGER)).unwrap();
    write_owned_ledger(&applier, "{ not json");
    assert!(matches!(
        applier.clear(),
        Err(ProjectionError::LedgerJson { .. })
    ));
    for entry in [
        "harness.json",
        "../outside.md",
        "/etc/hosts",
        "agents/../harness.json",
    ] {
        write_owned_ledger(&applier, &json!({ "files": [entry] }).to_string());
        assert!(
            matches!(applier.clear(), Err(ProjectionError::LedgerEntry { .. })),
            "{entry}"
        );
    }
    fs::remove_file(applier.config_dir().join(OWNED_LEDGER)).unwrap();
    let listed = applier.config_dir().join(SKILLS_DIR).join("who-am-i");
    fs::create_dir_all(&listed).unwrap();
    fs::write(listed.join("SKILL.md"), "x").unwrap();
    write_skills_ledger(&applier, r#"{"version":"x","ids":["who-am-i"]}"#);
    applier.clear().unwrap();
    assert!(!listed.exists());
    assert_eq!(fixtures::tree(applier.config_dir()), Vec::<String>::new());
}

#[test]
fn managed_directories_are_what_the_projection_can_name() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    for good in ["who-am-i", "a1"] {
        write_skills_ledger(
            &applier,
            &json!({ "version": "x", "ids": [good] }).to_string(),
        );
        assert!(applier.clear().is_ok(), "{good}");
    }
    let long = "a".repeat(65);
    for bad in ["", "-x", "Upper", "a_b", "a.b", "../x", long.as_str()] {
        write_skills_ledger(
            &applier,
            &json!({ "version": "x", "ids": [bad] }).to_string(),
        );
        assert!(
            matches!(applier.clear(), Err(ProjectionError::LedgerEntry { .. })),
            "{bad}"
        );
    }
}
