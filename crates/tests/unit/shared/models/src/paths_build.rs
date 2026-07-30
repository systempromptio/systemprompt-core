//! Tests for `BuildPaths` binary resolution, including debug/release sibling
//! preference by modification time.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use systemprompt_models::paths::{BuildPaths, PathError};
use systemprompt_models::profile::PathsConfig;
use tempfile::TempDir;

fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn build_paths(bin: &Path) -> BuildPaths {
    BuildPaths::from_profile(&PathsConfig {
        system: "/tmp".to_owned(),
        services: "/tmp".to_owned(),
        bin: bin.to_string_lossy().into_owned(),
        web_path: None,
        storage: None,
        geoip_database: None,
    })
}

fn touch(path: &Path) {
    fs::write(path, b"#!/bin/sh\n").unwrap();
}

// The resolver compares mtimes, so the two candidates must be written with a
// gap the filesystem can actually distinguish.
fn touch_then_touch(older: &Path, newer: &Path) {
    touch(older);
    std::thread::sleep(Duration::from_millis(20));
    touch(newer);
    assert!(
        fs::metadata(newer).unwrap().modified().unwrap()
            > fs::metadata(older).unwrap().modified().unwrap(),
        "filesystem mtime resolution too coarse to order these writes"
    );
}

#[test]
fn resolves_binary_in_primary_bin_dir() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("release");
    fs::create_dir_all(&bin).unwrap();
    touch(&bin.join(exe("tool")));

    let resolved = build_paths(&bin).resolve_binary("tool").unwrap();
    assert_eq!(resolved, bin.join(exe("tool")));
}

#[test]
fn missing_binary_reports_every_searched_path() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("release");
    fs::create_dir_all(&bin).unwrap();

    let err = build_paths(&bin).resolve_binary("ghost").unwrap_err();
    match err {
        PathError::BinaryNotFound { name, searched } => {
            assert_eq!(name, "ghost");
            assert!(
                searched.contains(&bin.join(exe("ghost"))),
                "primary path missing from {searched:?}"
            );
            assert!(
                searched
                    .iter()
                    .any(|p| p.starts_with(tmp.path().join("debug"))),
                "sibling debug path missing from {searched:?}"
            );
        },
        other => panic!("expected BinaryNotFound, got {other:?}"),
    }
}

#[test]
fn falls_back_to_debug_sibling_when_primary_absent() {
    let tmp = TempDir::new().unwrap();
    let release = tmp.path().join("release");
    let debug = tmp.path().join("debug");
    fs::create_dir_all(&release).unwrap();
    fs::create_dir_all(&debug).unwrap();
    touch(&debug.join(exe("tool")));

    let resolved = build_paths(&release).resolve_binary("tool").unwrap();
    assert_eq!(resolved, debug.join(exe("tool")));
}

#[test]
fn falls_back_to_release_sibling_when_running_from_debug() {
    let tmp = TempDir::new().unwrap();
    let release = tmp.path().join("release");
    let debug = tmp.path().join("debug");
    fs::create_dir_all(&release).unwrap();
    fs::create_dir_all(&debug).unwrap();
    touch(&release.join(exe("tool")));

    let resolved = build_paths(&debug).resolve_binary("tool").unwrap();
    assert_eq!(resolved, release.join(exe("tool")));
}

#[test]
fn newer_sibling_wins_over_older_primary() {
    let tmp = TempDir::new().unwrap();
    let release = tmp.path().join("release");
    let debug = tmp.path().join("debug");
    fs::create_dir_all(&release).unwrap();
    fs::create_dir_all(&debug).unwrap();
    touch_then_touch(&release.join(exe("tool")), &debug.join(exe("tool")));

    let resolved = build_paths(&release).resolve_binary("tool").unwrap();
    assert_eq!(
        resolved,
        debug.join(exe("tool")),
        "the more recently built sibling should win"
    );
}

#[test]
fn older_sibling_loses_to_newer_primary() {
    let tmp = TempDir::new().unwrap();
    let release = tmp.path().join("release");
    let debug = tmp.path().join("debug");
    fs::create_dir_all(&release).unwrap();
    fs::create_dir_all(&debug).unwrap();
    touch_then_touch(&debug.join(exe("tool")), &release.join(exe("tool")));

    let resolved = build_paths(&release).resolve_binary("tool").unwrap();
    assert_eq!(resolved, release.join(exe("tool")));
}

#[test]
fn non_build_dir_name_has_no_sibling_candidate() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("usr-local-bin");
    fs::create_dir_all(&bin).unwrap();

    let err = build_paths(&bin).resolve_binary("tool").unwrap_err();
    match err {
        PathError::BinaryNotFound { searched, .. } => {
            assert_eq!(
                searched,
                vec![bin.join(exe("tool"))],
                "only the primary path should be searched"
            );
        },
        other => panic!("expected BinaryNotFound, got {other:?}"),
    }
}

#[test]
fn bin_accessor_returns_configured_dir() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("release");
    assert_eq!(build_paths(&bin).bin(), bin.as_path());
}

#[test]
fn resolve_self_returns_an_absolute_path() {
    let resolved = BuildPaths::resolve_self().unwrap();
    assert!(
        resolved.is_absolute(),
        "resolve_self must yield an absolute path, got {resolved:?}"
    );
    assert_eq!(resolved, PathBuf::from(std::env::current_exe().unwrap()));
}
