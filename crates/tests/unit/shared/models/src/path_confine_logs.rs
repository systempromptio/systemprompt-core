//! Tests for `path_confine::logs` — on-disk log discovery, lookup, tailing,
//! display-name shaping, and confinement to the logs directory.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_models::path_confine::logs::{
    LogFile, confined_logs_dir, display_names, find_log_file, list_log_files, read_log_lines,
};
use tempfile::TempDir;

fn canonical(dir: &Path) -> std::path::PathBuf {
    dir.canonicalize().unwrap()
}

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

    let found = find_log_file(dir.path(), "agent-", "agent name", "alpha").unwrap();
    assert_eq!(
        found.path().canonicalize().unwrap(),
        canonical(dir.path()).join("alpha.log")
    );
}

#[test]
fn find_log_file_falls_back_to_prefixed_name() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-alpha.log", "");

    let found = find_log_file(dir.path(), "agent-", "agent name", "alpha").unwrap();
    assert_eq!(
        found.path().canonicalize().unwrap(),
        canonical(dir.path()).join("agent-alpha.log")
    );
}

#[test]
fn find_log_file_falls_back_to_substring_match() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-alpha-worker.log", "");

    let found = find_log_file(dir.path(), "agent-", "agent name", "alpha").unwrap();
    assert_eq!(
        found.path().canonicalize().unwrap(),
        canonical(dir.path()).join("agent-alpha-worker.log")
    );
}

#[test]
fn find_log_file_reports_available_files_on_miss() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "agent-beta.log", "");

    let err = find_log_file(dir.path(), "agent-", "agent name", "alpha").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("alpha"));
    assert!(msg.contains("agent-beta.log"));
}

#[test]
fn read_log_lines_returns_last_n_lines() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.log", "one\ntwo\nthree\nfour\n");

    let lines = read_log_lines(open(dir.path(), "a.log"), 2, |_| true).unwrap();
    assert_eq!(lines, vec!["three", "four"]);
}

#[test]
fn read_log_lines_handles_short_files() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.log", "only\n");

    let lines = read_log_lines(open(dir.path(), "a.log"), 10, |_| true).unwrap();
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

    let lines = read_log_lines(open(dir.path(), "a.log"), 2, |l| l.starts_with("ERROR")).unwrap();
    assert_eq!(lines, vec!["ERROR three", "ERROR five"]);
}

#[test]
fn opening_a_missing_log_file_errors() {
    let dir = TempDir::new().unwrap();
    assert!(LogFile::open_within(dir.path(), &dir.path().join("missing.log")).is_err());
}

#[cfg(unix)]
#[test]
fn a_found_log_reads_through_its_held_handle_after_the_path_is_swapped() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(dir.path(), "agent-swap.log", "inside
");
    write(outside.path(), "secret.log", "secret
");

    let found = find_log_file(dir.path(), "agent-", "agent name", "swap").unwrap();
    fs::remove_file(dir.path().join("agent-swap.log")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.log"),
        dir.path().join("agent-swap.log"),
    )
    .unwrap();

    let lines = read_log_lines(found, 10, |_| true).unwrap();
    assert_eq!(lines, vec!["inside"], "check and open are one step");
}

#[cfg(unix)]
#[test]
fn opening_a_log_linked_out_of_the_logs_dir_is_refused() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "secret.log", "secret");
    std::os::unix::fs::symlink(outside.path().join("secret.log"), dir.path().join("a.log")).unwrap();

    let err = LogFile::open_within(dir.path(), &dir.path().join("a.log")).unwrap_err();
    assert!(err.to_string().contains("resolves outside"), "{err}");
}

fn open(dir: &std::path::Path, name: &str) -> LogFile {
    LogFile::open_within(dir, &dir.join(name)).unwrap()
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

#[test]
fn find_log_file_refuses_a_name_that_is_not_one_segment() {
    let root = TempDir::new().unwrap();
    let dir = root.path().join("logs");
    fs::create_dir(&dir).unwrap();
    write(root.path(), "escape.log", "secret");

    for name in ["../escape", "a/b", "..", ".", ""] {
        let err = find_log_file(&dir, "agent-", "agent name", name).unwrap_err();
        assert!(
            err.to_string().contains("not a single path segment"),
            "{name}: {err}"
        );
    }
}

#[cfg(unix)]
#[test]
fn find_log_file_refuses_a_log_linked_out_of_the_logs_dir() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "secret.log", "secret");
    std::os::unix::fs::symlink(
        outside.path().join("secret.log"),
        dir.path().join("agent-leak.log"),
    )
    .unwrap();

    let err = find_log_file(dir.path(), "agent-", "agent name", "leak").unwrap_err();
    assert!(err.to_string().contains("resolves outside"), "{err}");
    let err = list_log_files(dir.path(), "agent-").unwrap_err();
    assert!(err.to_string().contains("resolves outside"), "{err}");
}

#[test]
fn confined_logs_dir_resolves_inside_the_root_and_refuses_outside_it() {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join("archive")).unwrap();
    let outside = TempDir::new().unwrap();

    assert_eq!(confined_logs_dir(root.path(), None).unwrap(), root.path());
    assert_eq!(
        confined_logs_dir(root.path(), Some("archive")).unwrap(),
        canonical(root.path()).join("archive")
    );
    for requested in [outside.path().to_str().unwrap(), "..", "missing"] {
        let err = confined_logs_dir(root.path(), Some(requested)).unwrap_err();
        assert!(
            err.to_string().contains("must name a directory inside"),
            "{requested}: {err}"
        );
    }
}

#[cfg(unix)]
#[test]
fn confined_logs_dir_refuses_a_link_out_of_the_root() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();

    let err = confined_logs_dir(root.path(), Some("link")).unwrap_err();
    assert!(
        err.to_string().contains("must name a directory inside"),
        "{err}"
    );
}
