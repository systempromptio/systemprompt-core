//! Tests for `analytics::shared::export` — CSV serialization, field
//! escaping, directory creation, and path resolution.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use serde::Serialize;
use systemprompt_cli::analytics::shared::export::{
    CsvBuilder, ensure_export_dir, export_single_to_csv, export_to_csv, resolve_export_path,
};

#[derive(Serialize)]
struct Row {
    name: String,
    count: u32,
    note: Option<String>,
}

fn row(name: &str, count: u32, note: Option<&str>) -> Row {
    Row {
        name: name.to_owned(),
        count,
        note: note.map(str::to_owned),
    }
}

#[test]
fn resolve_export_path_keeps_absolute_and_parented_paths() {
    let abs = Path::new("/tmp/exports/out.csv");
    assert_eq!(resolve_export_path(abs).unwrap(), abs);

    let rel = Path::new("subdir/out.csv");
    assert_eq!(resolve_export_path(rel).unwrap(), rel);
}

#[test]
fn export_to_csv_writes_headers_escaped_fields_and_nulls() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nested/dir/out.csv");

    let rows = vec![
        row("plain", 1, Some("hello")),
        row("with,comma", 2, None),
        row("say \"hi\"", 3, Some("line\nbreak")),
    ];
    export_to_csv(&rows, &path).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines[0], "name,count,note");
    assert_eq!(lines[1], "plain,1,hello");
    assert_eq!(lines[2], "\"with,comma\",2,");
    assert!(lines[3].starts_with("\"say \"\"hi\"\"\",3,\"line"));
    assert_eq!(lines[4], "break\"");
}

#[test]
fn export_to_csv_empty_slice_writes_empty_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("empty.csv");

    export_to_csv::<Row>(&[], &path).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "");
}

#[test]
fn export_single_to_csv_writes_one_record() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("single.csv");

    export_single_to_csv(&row("only", 9, Some("v")), &path).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "name,count,note\nonly,9,v\n");
}

#[test]
fn ensure_export_dir_creates_missing_parents_only() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("a/b/c.csv");
    ensure_export_dir(&path).unwrap();
    assert!(tmp.path().join("a/b").is_dir());

    ensure_export_dir(Path::new("bare.csv")).unwrap();
}

#[test]
fn csv_builder_writes_headers_and_escaped_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("built/out.csv");

    let mut builder = CsvBuilder::default().headers(vec!["id", "label"]);
    builder.add_row(vec!["1".to_owned(), "simple".to_owned()]);
    builder.add_row(vec!["2".to_owned(), "needs,quotes".to_owned()]);
    builder.write_to_file(&path).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "id,label\n1,simple\n2,\"needs,quotes\"\n");
}
