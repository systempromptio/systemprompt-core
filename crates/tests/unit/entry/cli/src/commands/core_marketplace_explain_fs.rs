//! Tests for `core marketplace explain`.
//!
//! `explain` dry-runs the bridge manifest assembly against the profile's
//! services tree and reports, per catalogue entry, whether it was delivered
//! and which stage dropped it. It touches no database — only the profile, the
//! services config and the tempdir the bootstrap fixture populates — so the
//! whole arm is reachable by seeding files.
//!
//! These assert on the returned `CommandOutput` rather than on `execute`,
//! which only renders to stdout. An earlier draft drove `execute` and checked
//! `Ok(())`; it passed with the kind filter deliberately broken, which is no
//! test at all.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use serde_json::Value;
use systemprompt_cli::core::marketplace::{ExplainArgs, explain};

const PLUGIN_YAML: &str = r#"plugin:
  id: explainplugin
  name: Explain Plugin
  description: Fixture plugin for manifest explain
  version: 1.0.0
  author:
    name: Tester
    email: tester@example.com
  keywords: [demo]
  license: MIT
  category: tools
  skills:
    source: explicit
    include: [explainskill]
  agents:
    source: explicit
    include: []
"#;

fn seed_catalogue() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let root = boot.services_path.clone();

    let skill = root.join("skills/explainskill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: explainskill\nname: Explain Skill\ndescription: A fixture skill\n",
    )
    .unwrap();
    std::fs::write(skill.join("index.md"), "# Explain Skill\n\nBody.\n").unwrap();

    let plugin = root.join("plugins/explainplugin");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("config.yaml"), PLUGIN_YAML).unwrap();

    root
}

fn args(skill: Option<&str>, plugin: Option<&str>) -> ExplainArgs {
    ExplainArgs {
        skill: skill.map(ToOwned::to_owned),
        plugin: plugin.map(ToOwned::to_owned),
        user: None,
    }
}

async fn rows(args: &ExplainArgs) -> Vec<Value> {
    let output = explain(args).await.expect("explain");
    let artifact = serde_json::to_value(output.artifact()).expect("serialise artifact");
    artifact["items"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("no items in artifact: {artifact}"))
}

fn ids_of_kind(rows: &[Value], kind: &str) -> Vec<String> {
    rows.iter()
        .filter(|r| r["kind"] == kind)
        .map(|r| r["id"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn explain_reports_the_seeded_catalogue() {
    seed_catalogue();

    let rows = rows(&args(None, None)).await;

    assert!(
        ids_of_kind(&rows, "skill")
            .iter()
            .any(|id| id == "explainskill"),
        "the seeded skill should appear in the explain table: {rows:?}"
    );
    assert!(
        ids_of_kind(&rows, "plugin")
            .iter()
            .any(|id| id == "explainplugin"),
        "the seeded plugin should appear in the explain table: {rows:?}"
    );
}

#[tokio::test]
async fn every_row_says_whether_it_was_delivered() {
    seed_catalogue();

    let rows = rows(&args(None, None)).await;
    assert!(!rows.is_empty(), "expected at least the seeded entries");

    for row in &rows {
        let delivered = row["delivered"]
            .as_bool()
            .unwrap_or_else(|| panic!("row has no boolean `delivered`: {row}"));
        // A dropped row must name the stage that dropped it, or the command
        // reports a rejection nobody can act on.
        if !delivered {
            assert!(
                !row["dropped_at"].as_str().unwrap_or_default().is_empty(),
                "a dropped row must name its stage: {row}"
            );
        }
    }
}

#[tokio::test]
async fn the_skill_filter_selects_only_that_skill() {
    seed_catalogue();

    let rows = rows(&args(Some("explainskill"), None)).await;

    assert_eq!(
        ids_of_kind(&rows, "skill"),
        vec!["explainskill".to_owned()],
        "only the named skill should survive the filter"
    );
    assert!(
        rows.iter().all(|r| r["kind"] == "skill"),
        "the skill filter must drop every other kind: {rows:?}"
    );
}

#[tokio::test]
async fn the_plugin_filter_selects_only_that_plugin() {
    seed_catalogue();

    let rows = rows(&args(None, Some("explainplugin"))).await;

    assert_eq!(
        ids_of_kind(&rows, "plugin"),
        vec!["explainplugin".to_owned()],
        "only the named plugin should survive the filter"
    );
    assert!(
        rows.iter().all(|r| r["kind"] == "plugin"),
        "the plugin filter must drop every other kind: {rows:?}"
    );
}

// Why: the filters match on kind AND id. A filter that compared id alone would
// return the plugin here and report it under `--skill`, which is how a
// diagnostic command starts lying about what it found.
#[tokio::test]
async fn a_filter_naming_the_other_kind_selects_nothing() {
    seed_catalogue();

    assert!(
        rows(&args(Some("explainplugin"), None)).await.is_empty(),
        "a plugin id under --skill must select nothing"
    );
    assert!(
        rows(&args(None, Some("explainskill"))).await.is_empty(),
        "a skill id under --plugin must select nothing"
    );
}

#[tokio::test]
async fn an_unknown_id_selects_nothing_rather_than_erroring() {
    seed_catalogue();

    assert!(
        rows(&args(Some("no_such_skill_at_all"), None))
            .await
            .is_empty(),
        "an unknown id is an empty report, not a failure"
    );
}
