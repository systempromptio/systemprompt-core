//! Reachable error and fallback arms of the analytics export helpers.
//!
//! The command tests call these only along their happy paths; the arms here
//! are the ones an export hits when the environment is wrong.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use crate::boot_ctx::bootstrap_ctx;
use serde::Serialize;
use systemprompt_analytics_entry::cli::shared::export::{
    ensure_export_dir, export_to_csv, resolve_export_path,
};

#[derive(Serialize)]
struct Row {
    name: String,
    count: u32,
}

#[test]
fn an_absolute_export_path_is_returned_unchanged() {
    let absolute = PathBuf::from("/tmp/cov-export/report.csv");

    assert_eq!(
        resolve_export_path(&absolute, &bootstrap_ctx()).unwrap(),
        absolute
    );
}

#[test]
fn a_parented_relative_export_path_is_returned_unchanged() {
    let parented = PathBuf::from("reports/cov/report.csv");

    assert_eq!(
        resolve_export_path(&parented, &bootstrap_ctx()).unwrap(),
        parented
    );
}

#[test]
fn a_bare_filename_is_resolved_under_the_profile_exports_directory() {
    let resolved = resolve_export_path(Path::new("report.csv"), &bootstrap_ctx()).unwrap();

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
