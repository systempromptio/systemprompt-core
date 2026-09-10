//! Tests for `core services validate --against`.
//!
//! The drift check is the reason `--against` exists: a plugin whose files
//! changed while its version stayed put ships different content under a name
//! consumers already pinned.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::core::services::bundle::{BundleArgs, pack_bundle};
use systemprompt_cli::core::services::validate::{ValidateArgs, run};

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("relative path has a parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

fn plugin_tree(root: &Path, version: &str, skill_body: &str) {
    write(
        root,
        "plugins/alpha/config.yaml",
        &format!("plugin:\n  id: alpha\n  version: {version}\n"),
    );
    write(root, "plugins/alpha/skills/report.md", skill_body);
}

fn pack(root: &Path, out: PathBuf) -> PathBuf {
    pack_bundle(
        &BundleArgs {
            root: root.to_path_buf(),
            out: out.clone(),
            version: "1.0.0".to_owned(),
            sign_key: None,
            source_repo: None,
            source_commit: None,
            workflow_run: None,
            marketplace_only: false,
        },
        None,
    )
    .expect("pack succeeds");
    out
}

fn versions_detail(root: &Path, against: PathBuf) -> String {
    let findings = run(&ValidateArgs {
        root: root.to_path_buf(),
        base: None,
        against: Some(against),
        strict: false,
    })
    .expect("validate runs");

    findings
        .iter()
        .find(|f| f.check == "versions")
        .map(|f| format!("{}:{}", f.status, f.detail))
        .expect("a versions finding is always emitted with --against")
}

#[test]
fn changed_files_without_a_version_bump_are_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let previous = dir.path().join("previous");
    plugin_tree(&previous, "1.0.0", "first\n");
    let archive = pack(&previous, dir.path().join("previous.tar.gz"));

    let current = dir.path().join("current");
    plugin_tree(&current, "1.0.0", "second\n");

    let detail = versions_detail(&current, archive);
    assert!(detail.starts_with("warn:"), "expected a warning: {detail}");
    assert!(detail.contains("alpha"), "plugin not named: {detail}");
    assert!(detail.contains("1.0.0"), "version not named: {detail}");
}

#[test]
fn a_bumped_version_passes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let previous = dir.path().join("previous");
    plugin_tree(&previous, "1.0.0", "first\n");
    let archive = pack(&previous, dir.path().join("previous.tar.gz"));

    let current = dir.path().join("current");
    plugin_tree(&current, "1.1.0", "second\n");

    let detail = versions_detail(&current, archive);
    assert!(detail.starts_with("pass:"), "expected a pass: {detail}");
}

#[test]
fn an_unchanged_plugin_passes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let previous = dir.path().join("previous");
    plugin_tree(&previous, "1.0.0", "first\n");
    let archive = pack(&previous, dir.path().join("previous.tar.gz"));

    let current = dir.path().join("current");
    plugin_tree(&current, "1.0.0", "first\n");

    let detail = versions_detail(&current, archive);
    assert!(detail.starts_with("pass:"), "expected a pass: {detail}");
}
