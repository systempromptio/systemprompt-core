//! Golden assertions on the CLI's structured-output wire contract.
//!
//! Unlike the permissive coverage drivers in `subprocess_with_db.rs`, these
//! tests are strict: each command must exit successfully and its ENTIRE stdout
//! must parse as one JSON `CliArtifact` with the expected `artifact_type` tag
//! and structural fields. Downstream MCP servers deserialize stdout verbatim
//! (`serde_json::from_str::<CliArtifact>(&stdout)`), so any stray stdout line
//! or shape change here is a wire-contract break, not a cosmetic diff.
//!
//! Tests skip silently when `DATABASE_URL` is unset, matching the other
//! subprocess suites.

use assert_cmd::Command;
use serde_json::Value;

fn systemprompt_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_BIN") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        for sub in [
            "target/debug/systemprompt",
            "crates/tests/target/debug/systemprompt",
        ] {
            let candidate = ancestor.join(sub);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    panic!("systemprompt binary not found; set SYSTEMPROMPT_BIN or run via `just coverage`");
}

fn golden_json(args: &[&str]) -> Option<Value> {
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|u| !u.is_empty())?;
    let mut cmd = Command::new(systemprompt_bin());
    cmd.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    cmd.env_remove("RUST_LOG");
    cmd.arg("--json").arg("--database-url").arg(url).args(args);
    let assert = cmd.assert().success();
    let stdout =
        String::from_utf8(assert.get_output().stdout.clone()).expect("stdout must be valid UTF-8");
    let value: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("whole stdout must be one JSON document: {e}\n---\n{stdout}"));
    Some(value)
}

fn assert_artifact_type(value: &Value, expected: &str) {
    assert_eq!(
        value.get("artifact_type").and_then(Value::as_str),
        Some(expected),
        "unexpected artifact_type in {value}"
    );
}

#[test]
fn admin_users_count_emits_presentation_card() {
    let Some(v) = golden_json(&["admin", "users", "count"]) else {
        return;
    };
    assert_artifact_type(&v, "presentation_card");
    assert!(v.get("title").is_some(), "card must carry a title: {v}");
    assert!(
        v.get("sections").and_then(Value::as_array).is_some(),
        "card must carry sections: {v}"
    );
}

#[test]
fn admin_users_list_emits_table() {
    let Some(v) = golden_json(&["admin", "users", "list", "--limit", "5"]) else {
        return;
    };
    assert_artifact_type(&v, "table");
    assert!(
        v.get("columns").and_then(Value::as_array).is_some(),
        "table must carry columns: {v}"
    );
    assert!(
        v.get("items").and_then(Value::as_array).is_some(),
        "table must carry items: {v}"
    );
}

#[test]
fn analytics_overview_emits_presentation_card() {
    let Some(v) = golden_json(&["analytics", "overview"]) else {
        return;
    };
    assert_artifact_type(&v, "presentation_card");
    assert_eq!(
        v.get("title").and_then(Value::as_str),
        Some("Analytics Overview"),
        "overview card title is part of the contract: {v}"
    );
}

#[test]
fn core_content_list_emits_table() {
    let Some(v) = golden_json(&["core", "content", "list", "--limit", "5"]) else {
        return;
    };
    assert_artifact_type(&v, "table");
    assert!(
        v.get("columns").and_then(Value::as_array).is_some(),
        "table must carry columns: {v}"
    );
}

#[test]
fn core_files_stats_emits_presentation_card() {
    let Some(v) = golden_json(&["core", "files", "stats"]) else {
        return;
    };
    assert_artifact_type(&v, "presentation_card");
    assert!(
        v.get("sections").and_then(Value::as_array).is_some(),
        "card must carry sections: {v}"
    );
}

#[test]
fn infra_db_indexes_emits_table() {
    let Some(v) = golden_json(&["infra", "db", "indexes"]) else {
        return;
    };
    assert_artifact_type(&v, "table");
    assert!(
        v.get("columns").and_then(Value::as_array).is_some(),
        "table must carry columns: {v}"
    );
}

#[test]
fn yaml_output_is_well_formed_for_users_count() {
    let Some(url) = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) else {
        return;
    };
    let mut cmd = Command::new(systemprompt_bin());
    cmd.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    cmd.env_remove("RUST_LOG");
    cmd.args(["--yaml", "--database-url", &url, "admin", "users", "count"]);
    let assert = cmd.assert().success();
    let stdout =
        String::from_utf8(assert.get_output().stdout.clone()).expect("stdout must be valid UTF-8");
    let value: serde_yaml::Value = serde_yaml::from_str(&stdout)
        .unwrap_or_else(|e| panic!("whole stdout must be one YAML document: {e}\n---\n{stdout}"));
    assert_eq!(
        value
            .get("artifact_type")
            .and_then(serde_yaml::Value::as_str),
        Some("presentation_card"),
        "unexpected artifact_type in YAML output"
    );
}
