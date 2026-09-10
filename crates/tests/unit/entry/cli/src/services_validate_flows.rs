//! `core services validate` — composition, findings and the strict gate.
//!
//! A marketplace-only tree carries no `config/config.yaml`, so validating it
//! alone must say what to pass rather than report a broken tree. Composing it
//! over a base is the case CI actually runs, and a cross-bundle collision has
//! to surface here rather than at the next boot.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::core::services::bundle::{BundleArgs, pack_bundle};
use systemprompt_cli::core::services::validate::{ValidateArgs, execute, run};

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

fn platform_tree(root: &Path) {
    write(root, "config/config.yaml", "settings: {}\n");
}

fn marketplace_tree(root: &Path) {
    write(
        root,
        "skills/echo_skill/config.yaml",
        "id: echo_skill\nname: Echo\ndescription: fixture skill\n",
    );
    write(root, "skills/echo_skill/index.md", "# Echo\n");
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

fn args(root: &Path, base: Option<PathBuf>, strict: bool) -> ValidateArgs {
    ValidateArgs {
        root: root.to_path_buf(),
        base,
        against: None,
        strict,
    }
}

fn finding(
    findings: &[systemprompt_cli::core::services::validate::Finding],
    check: &str,
) -> String {
    findings
        .iter()
        .find(|f| f.check == check)
        .map(|f| format!("{}:{}", f.status, f.detail))
        .unwrap_or_else(|| panic!("no {check} finding in {findings:?}"))
}

#[test]
fn a_marketplace_only_tree_is_told_to_pass_a_base() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tree = dir.path().join("tree");
    marketplace_tree(&tree);

    let detail = finding(
        &run(&args(&tree, None, false)).expect("validate runs"),
        "config",
    );
    assert!(detail.starts_with("fail:"), "{detail}");
    assert!(detail.contains("--base"), "{detail}");
}

#[test]
fn a_tree_composed_over_a_base_reports_both_the_compose_and_the_catalog() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("base");
    platform_tree(&base);
    let archive = pack(&base, dir.path().join("base.tar.gz"));

    let tree = dir.path().join("tree");
    marketplace_tree(&tree);

    let findings = run(&args(&tree, Some(archive), false)).expect("validate runs");
    assert!(finding(&findings, "compose").starts_with("pass:"));
    assert!(
        finding(&findings, "config").starts_with("pass:"),
        "{findings:?}"
    );
    assert!(finding(&findings, "catalog").starts_with("pass:"));
}

#[test]
fn two_bundles_claiming_the_same_file_fail_to_compose() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("base");
    platform_tree(&base);
    marketplace_tree(&base);
    let archive = pack(&base, dir.path().join("base.tar.gz"));

    let tree = dir.path().join("tree");
    marketplace_tree(&tree);

    let findings = run(&args(&tree, Some(archive), false)).expect("validate runs");
    let detail = finding(&findings, "compose");
    assert!(detail.starts_with("fail:"), "{detail}");
    assert_eq!(
        findings.len(),
        1,
        "a failed compose must stop the run: {findings:?}"
    );
}

#[test]
fn a_broken_config_is_a_failing_finding_not_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tree = dir.path().join("tree");
    write(&tree, "config/config.yaml", "settings: [not, a, map\n");

    let detail = finding(
        &run(&args(&tree, None, false)).expect("validate runs"),
        "config",
    );
    assert!(detail.starts_with("fail:"), "{detail}");
}

#[test]
fn a_failing_check_makes_the_command_report_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tree = dir.path().join("tree");
    marketplace_tree(&tree);

    let (output, ok) = execute(&args(&tree, None, false)).expect("validate runs");
    assert!(!ok, "a missing config must not pass");
    assert_eq!(output.title(), Some("Services Validation"));
    let body = serde_json::to_string(output.artifact()).expect("output serialises");
    assert!(body.contains("--base"), "{body}");
}

#[test]
fn a_clean_tree_passes_and_strictness_does_not_change_that() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tree = dir.path().join("tree");
    platform_tree(&tree);

    assert!(execute(&args(&tree, None, false)).expect("validate runs").1);
    assert!(execute(&args(&tree, None, true)).expect("validate runs").1);
}
