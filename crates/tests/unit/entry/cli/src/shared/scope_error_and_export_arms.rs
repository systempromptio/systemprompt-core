//! Tests for reachable error and fallback arms in shared helpers.
//!
//! These functions are called by the command tests, but only along their happy
//! paths; the arms here are the ones a command hits when the environment is
//! wrong.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use serde::Serialize;
use systemprompt_cli::analytics::shared::export::{
    ensure_export_dir, export_to_csv, resolve_export_path,
};
use systemprompt_cli::shared::database_scoped_command_error;

#[derive(Serialize)]
struct Row {
    name: String,
    count: u32,
}

#[test]
fn the_database_scope_error_names_the_active_profile_once_one_is_installed() {
    systemprompt_test_fixtures::ensure_test_bootstrap();

    let rendered = database_scoped_command_error().to_string();

    assert!(
        rendered.contains("routes to an external/cloud database"),
        "{rendered}"
    );
    assert!(rendered.contains("--profile <local-profile>"), "{rendered}");
}

#[test]
fn an_absolute_export_path_is_returned_unchanged() {
    let absolute = PathBuf::from("/tmp/cov-export/report.csv");

    assert_eq!(resolve_export_path(&absolute).unwrap(), absolute);
}

#[test]
fn a_parented_relative_export_path_is_returned_unchanged() {
    let parented = PathBuf::from("reports/cov/report.csv");

    assert_eq!(resolve_export_path(&parented).unwrap(), parented);
}

#[test]
fn a_bare_filename_is_resolved_under_the_profile_exports_directory() {
    systemprompt_test_fixtures::ensure_test_bootstrap();

    let resolved = resolve_export_path(Path::new("report.csv")).unwrap();

    assert!(resolved.is_absolute(), "{}", resolved.display());
    assert!(resolved.ends_with("report.csv"), "{}", resolved.display());
    assert_ne!(resolved, PathBuf::from("report.csv"));
}

#[test]
fn the_export_directory_is_created_only_when_it_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("a/b/c/report.csv");

    ensure_export_dir(&nested).unwrap();
    assert!(nested.parent().unwrap().is_dir());

    // A second call is a no-op rather than an error.
    ensure_export_dir(&nested).unwrap();
    assert!(nested.parent().unwrap().is_dir());
}

#[test]
fn creating_an_export_directory_under_a_file_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, "not a directory").unwrap();

    let err = ensure_export_dir(&blocker.join("nested/report.csv")).unwrap_err();
    assert!(
        format!("{err:#}").contains("Failed to create export directory"),
        "{err:#}"
    );
}

#[test]
fn writing_a_csv_into_an_unwritable_location_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, "not a directory").unwrap();

    let rows = vec![Row {
        name: "alpha".to_owned(),
        count: 1,
    }];

    let err = export_to_csv(&rows, &blocker.join("report.csv")).unwrap_err();
    assert!(!format!("{err:#}").is_empty());
}

#[test]
fn a_csv_export_escapes_separators_and_renders_every_row() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("rows.csv");

    let rows = vec![
        Row {
            name: "plain".to_owned(),
            count: 1,
        },
        Row {
            name: "has,comma".to_owned(),
            count: 2,
        },
    ];

    export_to_csv(&rows, &path).unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = csv.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 3, "{csv}");
    assert_eq!(lines[0], "name,count");
    assert!(lines[2].contains("\"has,comma\""), "{csv}");
}
