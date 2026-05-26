//! Tests for `commands::cloud::profile::templates` — the small filesystem
//! helpers that have no profile/db dependency.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::templates::{
    get_services_path, save_dockerignore, save_entrypoint,
};
use tempfile::TempDir;

#[test]
fn save_dockerignore_writes_file() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join(".dockerignore");
    save_dockerignore(&path).expect("writes");
    let content = std::fs::read_to_string(&path).expect("readable");
    assert!(content.contains(".git"));
    assert!(content.contains("target/debug"));
}

#[test]
fn save_dockerignore_creates_parent_dir() {
    let dir = TempDir::new().expect("tempdir");
    let nested = dir.path().join("a/b/c/.dockerignore");
    save_dockerignore(&nested).expect("writes");
    assert!(nested.exists());
}

#[test]
fn save_entrypoint_writes_executable_script() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("entrypoint.sh");
    save_entrypoint(&path).expect("writes");
    let content = std::fs::read_to_string(&path).expect("readable");
    assert!(content.starts_with("#!/bin/sh"));
    assert!(content.contains("systemprompt"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "entrypoint should be 0755");
    }
}

#[test]
fn save_entrypoint_creates_parent_dir() {
    let dir = TempDir::new().expect("tempdir");
    let nested = dir.path().join("docker/entrypoint.sh");
    save_entrypoint(&nested).expect("writes");
    assert!(nested.exists());
}

#[test]
fn get_services_path_returns_a_path() {
    let p = get_services_path().expect("returns path");
    assert!(!p.is_empty());
}
