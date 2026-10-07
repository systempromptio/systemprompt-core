//! Authored `SKILL.md` frontmatter survives kit import, the catalog and the
//! rendered client `SKILL.md`; the platform-owned keys never reach it twice.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_manifest::services::split_frontmatter;
use systemprompt_marketplace::bundle::{BundleContent, build_plugin_bundle};
use systemprompt_marketplace::catalog::load_skills;
use systemprompt_marketplace::{ImportOptions, import_anthropic_tree};
use systemprompt_models::bridge::manifest::SkillEntry;
use systemprompt_models::plugin::PluginComponentRef;
use tempfile::TempDir;

use crate::bundle::{explicit, plugin_config};

const AUTHORED: &str = "---
name: field-notes
title: Field Notes
description: Turn engagement notes into a report.
argument-hint: \"[account] [quarter]\"
allowed-tools:
  - Read
  - Bash(git log *)
disable-model-invocation: true
tags: [notes, field]
hooks:
  PreToolUse:
    - matcher: Bash
      hooks:
        - type: command
          command: ./check.sh
          timeout: 30
metadata:
  owner: field-team
---

Write the report.
";

const PASS_THROUGH: [&str; 5] = [
    "argument-hint",
    "allowed-tools",
    "disable-model-invocation",
    "hooks",
    "metadata",
];

fn kit(skill_md: &str) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let write = |rel: &str, body: &str| {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("write");
    };
    write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","owner":{"name":"Acme"},"plugins":[{"name":"alpha","source":"./plugins/alpha"}]}"#,
    );
    write(
        "plugins/alpha/.claude-plugin/plugin.json",
        r#"{"name":"alpha","version":"1.0.0"}"#,
    );
    write("plugins/alpha/skills/field-notes/SKILL.md", skill_md);
    dir
}

fn import(skill_md: &str) -> Result<TempDir, String> {
    let source = kit(skill_md);
    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(
        source.path(),
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .map_err(|e| e.to_string())?;
    Ok(dest)
}

fn rendered(skills: &[SkillEntry]) -> String {
    let disabled = BTreeSet::new();
    let content = BundleContent {
        skills,
        rules: &[],
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &disabled,
        artifacts: &[],
        plugins_root: Path::new("/nonexistent"),
        managed_files: &BTreeMap::new(),
    };
    let config = plugin_config(
        "alpha",
        explicit(&["field_notes"]),
        PluginComponentRef::default(),
    );
    let bundle = build_plugin_bundle(&config, &content).expect("bundle builds");
    String::from_utf8(bundle["skills/field-notes/SKILL.md"].bytes.clone()).expect("utf8")
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn keys(mapping: &serde_yaml::Mapping) -> Vec<&str> {
    mapping
        .keys()
        .filter_map(serde_yaml::Value::as_str)
        .collect()
}

#[test]
fn authored_keys_reach_config_catalog_and_client_skill_md() {
    let dest = import(AUTHORED).expect("import succeeds");
    let config: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(dest.path().join("skills/field_notes/config.yaml"))
            .expect("config written"),
    )
    .expect("config yaml");
    let recorded = config["frontmatter"]
        .as_mapping()
        .expect("frontmatter kept");
    assert_eq!(
        keys(recorded),
        PASS_THROUGH,
        "authored order, owned keys removed"
    );
    assert_eq!(config["name"], "Field Notes");
    assert_eq!(config["tags"][1], "field");

    let skills = load_skills(dest.path()).expect("catalog loads");
    let skill = &skills[0];
    assert_eq!(
        keys(skill.frontmatter.as_ref().expect("catalog carries it")),
        PASS_THROUGH
    );
    assert_ne!(
        skill.sha256.as_str(),
        hex_sha256(skill.instructions.as_bytes()),
        "the skill digest covers the pass-through frontmatter"
    );

    let md = rendered(&skills);
    let front = split_frontmatter(&md).expect("frontmatter");
    let parsed: serde_yaml::Value = serde_yaml::from_str(front.yaml).expect("valid yaml");
    assert_eq!(parsed["name"], "field-notes");
    assert_eq!(
        parsed["description"],
        "Turn engagement notes into a report."
    );
    assert_eq!(parsed["argument-hint"], "[account] [quarter]");
    assert_eq!(parsed["allowed-tools"][1], "Bash(git log *)");
    assert_eq!(parsed["disable-model-invocation"], true);
    assert_eq!(parsed["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"], 30);
    assert_eq!(parsed["metadata"]["owner"], "field-team");
    for owned in ["title", "tags", "category", "display_category", "hosts"] {
        assert!(
            parsed.get(owned).is_none(),
            "{owned} is platform-owned: {md}"
        );
    }
    assert_eq!(front.yaml.matches("\nname:").count(), 0);
    assert!(front.yaml.starts_with("name: field-notes\n"));
    assert_eq!(front.yaml.matches("description:").count(), 1);
    assert_eq!(front.body.trim(), "Write the report.");
}

#[test]
fn a_skill_with_only_name_and_description_renders_and_hashes_as_before() {
    let dest = import("---\nname: field-notes\ndescription: Just this.\n---\n\nBody.\n")
        .expect("import succeeds");
    let config = std::fs::read_to_string(dest.path().join("skills/field_notes/config.yaml"))
        .expect("config written");
    assert!(!config.contains("frontmatter"), "{config}");
    let skills = load_skills(dest.path()).expect("catalog loads");
    assert!(skills[0].frontmatter.is_none());
    assert_eq!(skills[0].sha256.as_str(), hex_sha256(b"Body."));
    assert_eq!(
        rendered(&skills),
        "---\nname: field-notes\ndescription: \"Just this.\"\n---\n\nBody.\n"
    );
}

#[test]
fn a_frontmatter_the_signed_manifest_cannot_carry_is_refused_at_import() {
    for bad in [
        "metadata:\n  1: one\n",
        "metadata: !custom tagged\n",
        "limits: .nan\n",
    ] {
        let md = format!("---\nname: field-notes\ndescription: d\n{bad}---\n\nBody.\n");
        let err = import(&md).expect_err("refused");
        assert!(
            err.contains("SKILL.md") && err.contains("frontmatter"),
            "{bad}: {err}"
        );
    }
    let err = import("---\n- not\n- a mapping\n---\n\nBody.\n").expect_err("refused");
    assert!(err.contains("must be a YAML mapping"), "{err}");
}

#[test]
fn the_manifest_json_round_trips_the_frontmatter() {
    let dest = import(AUTHORED).expect("import succeeds");
    let skills = load_skills(dest.path()).expect("catalog loads");
    let json = serde_json::to_string(&skills[0]).expect("serialise");
    let back: SkillEntry = serde_json::from_str(&json).expect("parse");
    assert_eq!(back.frontmatter, skills[0].frontmatter);
    let bare = "{\"id\":\"s\",\"name\":\"s\",\"description\":\"d\",\"file_path\":\"f\",\
                \"sha256\":\"0000000000000000000000000000000000000000000000000000000000000000\",\
                \"instructions\":\"i\"}";
    let old: SkillEntry = serde_json::from_str(bare).expect("a manifest without the field parses");
    assert!(old.frontmatter.is_none());
    assert!(
        !serde_json::to_string(&old)
            .expect("serialise")
            .contains("frontmatter")
    );
}
