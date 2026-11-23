//! Tests for `shared::disk_logs` — on-disk log discovery, lookup, tailing,
//! and display-name shaping.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::shared::disk_logs::{
    display_names, find_log_file, list_log_files, read_log_lines,
};
use tempfile::TempDir;

fn write(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

#[test]
fn list_log_files_filters_by_prefix_and_extension() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-alpha.log", "");
    write(dir.path(), "agent-beta.LOG", "");
    write(dir.path(), "mcp-content.log", "");
    write(dir.path(), "agent-notes.txt", "");
    write(dir.path(), "server.log", "");

    let files = list_log_files(dir.path(), "agent-").unwrap();
    assert_eq!(files, vec!["agent-alpha.log", "agent-beta.LOG"]);
}

#[test]
fn list_log_files_sorts_results() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "mcp-zeta.log", "");
    write(dir.path(), "mcp-alpha.log", "");

    let files = list_log_files(dir.path(), "mcp-").unwrap();
    assert_eq!(files, vec!["mcp-alpha.log", "mcp-zeta.log"]);
}

#[test]
fn list_log_files_errors_on_missing_dir() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nope");
    assert!(list_log_files(&missing, "agent-").is_err());
}

#[test]
fn find_log_file_prefers_exact_name() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "alpha.log", "");
    write(dir.path(), "agent-alpha.log", "");

    let found = find_log_file(dir.path(), "agent-", "alpha").unwrap();
    assert_eq!(found, dir.path().join("alpha.log"));
}

#[test]
fn find_log_file_falls_back_to_prefixed_name() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-alpha.log", "");

    let found = find_log_file(dir.path(), "agent-", "alpha").unwrap();
    assert_eq!(found, dir.path().join("agent-alpha.log"));
}

#[test]
fn find_log_file_falls_back_to_substring_match() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-alpha-worker.log", "");

    let found = find_log_file(dir.path(), "agent-", "alpha").unwrap();
    assert_eq!(found, dir.path().join("agent-alpha-worker.log"));
}

#[test]
fn find_log_file_reports_available_files_on_miss() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-beta.log", "");

    let err = find_log_file(dir.path(), "agent-", "alpha").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("alpha"));
    assert!(msg.contains("agent-beta.log"));
}

#[test]
fn read_log_lines_returns_last_n_lines() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.log", "one\ntwo\nthree\nfour\n");

    let lines = read_log_lines(&dir.path().join("a.log"), 2, |_| true).unwrap();
    assert_eq!(lines, vec!["three", "four"]);
}

#[test]
fn read_log_lines_handles_short_files() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.log", "only\n");

    let lines = read_log_lines(&dir.path().join("a.log"), 10, |_| true).unwrap();
    assert_eq!(lines, vec!["only"]);
}

#[test]
fn read_log_lines_applies_filter_before_tailing() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "a.log",
        "ERROR one\nINFO two\nERROR three\nINFO four\nERROR five\n",
    );

    let lines = read_log_lines(&dir.path().join("a.log"), 2, |l| l.starts_with("ERROR")).unwrap();
    assert_eq!(lines, vec!["ERROR three", "ERROR five"]);
}

#[test]
fn read_log_lines_errors_on_missing_file() {
    let dir = TempDir::new().unwrap();
    assert!(read_log_lines(&dir.path().join("missing.log"), 5, |_| true).is_err());
}

#[test]
fn display_names_strips_prefix_and_suffix() {
    let files = vec![
        "agent-alpha.log".to_owned(),
        "agent-beta.log".to_owned(),
        "other.log".to_owned(),
        "agent-raw".to_owned(),
    ];
    assert_eq!(
        display_names(&files, "agent-"),
        vec!["alpha", "beta", "other", "agent-raw"],
        "a name without the .log suffix falls back to the full file name"
    );
}
