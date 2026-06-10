#![allow(clippy::all)]

use std::path::Path;

use systemprompt_database::{SquashBaselineError, SquashBaselineService};

fn add_crate(root: &Path, layer: &str, extension_id: &str) {
    let crate_dir = root.join("crates").join(layer).join(extension_id);
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(crate_dir.join("Cargo.toml"), "[package]\n").unwrap();
}

fn mark_repo_root(root: &Path) {
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
}

#[test]
fn target_path_resolves_crate_from_nested_start_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mark_repo_root(root);
    add_crate(root, "domain", "users");
    let nested = root.join("crates").join("domain").join("users").join("src");
    std::fs::create_dir_all(&nested).unwrap();

    let path = SquashBaselineService::baseline_target_path(&nested, "users", 7).unwrap();

    assert_eq!(
        path,
        root.join("crates/domain/users/schema/migrations/000_baseline_v7.sql")
    );
}

#[test]
fn target_path_searches_all_layers() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mark_repo_root(root);
    add_crate(root, "entry", "cli");

    let path = SquashBaselineService::baseline_target_path(root, "cli", 3).unwrap();

    assert_eq!(
        path,
        root.join("crates/entry/cli/schema/migrations/000_baseline_v3.sql")
    );
}

#[test]
fn target_path_without_repo_markers_falls_back_to_start_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    add_crate(root, "infra", "events");

    let path = SquashBaselineService::baseline_target_path(root, "events", 2).unwrap();

    assert_eq!(
        path,
        root.join("crates/infra/events/schema/migrations/000_baseline_v2.sql")
    );
}

#[test]
fn target_path_unknown_extension_errors_with_tried_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mark_repo_root(root);
    std::fs::create_dir_all(root.join("crates")).unwrap();

    let err = SquashBaselineService::baseline_target_path(root, "ghost", 1).unwrap_err();

    match err {
        SquashBaselineError::ExtensionCrateNotFound {
            extension_id,
            tried,
        } => {
            assert_eq!(extension_id, "ghost");
            assert_eq!(tried.len(), 5);
        },
        other => panic!("expected ExtensionCrateNotFound, got: {other:?}"),
    }
}

#[test]
fn unknown_extension_error_message_names_the_layout() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mark_repo_root(root);
    std::fs::create_dir_all(root.join("crates")).unwrap();

    let err = SquashBaselineService::baseline_target_path(root, "ghost", 1).unwrap_err();

    let message = err.to_string();
    assert!(message.starts_with("Could not locate source crate for extension 'ghost'."));
    assert!(message.contains("crates/{layer}/{id}"));
    assert!(message.contains("write the baseline file by hand"));
}

#[test]
fn write_baseline_file_creates_parents_and_writes_sql() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("schema")
        .join("migrations")
        .join("000_baseline_v4.sql");

    SquashBaselineService::write_baseline_file(&path, "CREATE TABLE t (id TEXT);").unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written, "CREATE TABLE t (id TEXT);");
}

#[test]
fn write_baseline_file_overwrites_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("000_baseline_v1.sql");
    SquashBaselineService::write_baseline_file(&path, "old").unwrap();

    SquashBaselineService::write_baseline_file(&path, "new").unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[test]
fn write_baseline_file_blocked_parent_errors() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "not a directory").unwrap();
    let path = blocker.join("sub").join("000_baseline_v1.sql");

    let err = SquashBaselineService::write_baseline_file(&path, "sql").unwrap_err();

    assert!(matches!(err, SquashBaselineError::CreateDir { .. }));
    assert!(err.to_string().starts_with("Failed to create directory"));
}
