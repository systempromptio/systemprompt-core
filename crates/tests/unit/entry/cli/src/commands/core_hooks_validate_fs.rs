//! Filesystem-driven tests for `core hooks validate`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::core::hooks::validate::validate_all_hooks;

fn write_hook(root: &Path, dir: &str, yaml: &str) {
    let hook_dir = root.join(dir);
    fs::create_dir_all(&hook_dir).unwrap();
    fs::write(hook_dir.join("config.yaml"), yaml).unwrap();
}

#[test]
fn missing_hooks_dir_yields_empty_results() {
    let results = validate_all_hooks(Path::new("/nonexistent/hooks-root")).unwrap();
    assert!(results.is_empty());
}

#[test]
fn skips_files_and_dirs_without_config() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("stray.txt"), "x").unwrap();
    fs::create_dir_all(tmp.path().join("empty-dir")).unwrap();

    let results = validate_all_hooks(tmp.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn unparseable_config_reports_parse_error_under_dir_name() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(tmp.path(), "broken", "not: [valid: yaml");

    let results = validate_all_hooks(tmp.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].plugin_id, "broken");
    assert!(!results[0].valid);
    assert!(results[0].errors[0].contains("Failed to parse config.yaml"));
}

#[test]
fn empty_command_is_an_error_and_config_id_wins_over_dir_name() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(
        tmp.path(),
        "dir-name",
        "id: my_hook\nevent: PreToolUse\ncommand: \"\"\n",
    );

    let results = validate_all_hooks(tmp.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].plugin_id, "my_hook");
    assert!(!results[0].valid);
    assert_eq!(results[0].errors, vec!["command must not be empty"]);
}

#[test]
fn plugin_root_command_with_missing_script_is_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(
        tmp.path(),
        "rooted",
        "event: PreToolUse\ncommand: \"${CLAUDE_PLUGIN_ROOT}/scripts/run.sh\"\n",
    );

    let results = validate_all_hooks(tmp.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0].valid);
    assert!(results[0].errors[0].contains("missing script: scripts/run.sh"));
}

#[test]
fn plugin_root_command_with_existing_script_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(
        tmp.path(),
        "rooted",
        "event: PreToolUse\ncommand: \"${CLAUDE_PLUGIN_ROOT}/scripts/run.sh\"\n",
    );
    let scripts = tmp.path().join("rooted/scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(scripts.join("run.sh"), "#!/bin/sh\n").unwrap();

    let results = validate_all_hooks(tmp.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].valid);
    assert!(results[0].errors.is_empty());
    assert_eq!(results[0].plugin_id, "rooted");
}

#[test]
fn plain_command_without_plugin_root_var_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(
        tmp.path(),
        "plain",
        "event: PreToolUse\ncommand: \"echo hi\"\n",
    );

    let results = validate_all_hooks(tmp.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].valid);
}
