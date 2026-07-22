//! Tests for `plugins mcp list` server summaries and binary status.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::plugins::mcp::list::{determine_status, get_binary_info, summarize_server};
use systemprompt_models::mcp::Deployment;

fn deployment(yaml: &str) -> Deployment {
    serde_yaml::from_str(yaml).unwrap()
}

fn internal(binary: &str, enabled: bool) -> Deployment {
    deployment(&format!(
        "binary: \"{binary}\"\npackage: null\nport: 5010\nenabled: {enabled}\ndisplay_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  client_id: null\n"
    ))
}

fn external(enabled: bool) -> Deployment {
    deployment(&format!(
        "type: external\nbinary: \"\"\npackage: null\nport: 0\nendpoint: https://mcp.example.com/mcp\nenabled: {enabled}\ndisplay_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  client_id: null\n"
    ))
}

fn touch_binary(root: &Path, profile: &str, name: &str) {
    let dir = root.join("target").join(profile);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), "bin").unwrap();
}

#[test]
fn determine_status_covers_every_combination() {
    assert_eq!(determine_status(false, Some("d"), Some("r")), "disabled");
    assert_eq!(determine_status(true, Some("d"), Some("r")), "ready");
    assert_eq!(determine_status(true, Some("d"), None), "debug-only");
    assert_eq!(determine_status(true, None, Some("r")), "release-only");
    assert_eq!(determine_status(true, None, None), "not-built");
}

#[test]
fn get_binary_info_without_root_or_binary_is_empty() {
    assert_eq!(get_binary_info(None, "svc", false), (None, None));

    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(get_binary_info(Some(tmp.path()), "svc", true), (None, None));
}

#[test]
fn get_binary_info_reports_path_and_timestamp() {
    let tmp = tempfile::tempdir().unwrap();
    touch_binary(tmp.path(), "debug", "svc");

    let (path, created) = get_binary_info(Some(tmp.path()), "svc", false);
    assert!(path.unwrap().ends_with("target/debug/svc"));
    assert!(created.is_some());
}

#[test]
fn external_server_summary_reports_remote_or_disabled() {
    let running = summarize_server("ext", &external(true), None);
    assert_eq!(running.server_type, "external");
    assert_eq!(running.port, 0);
    assert_eq!(running.status.as_deref(), Some("remote"));
    assert_eq!(
        running.endpoint.as_deref(),
        Some("https://mcp.example.com/mcp")
    );
    assert!(running.binary_debug.is_none());

    let stopped = summarize_server("ext", &external(false), None);
    assert_eq!(stopped.status.as_deref(), Some("disabled"));
}

#[test]
fn internal_summary_falls_back_to_server_name_for_empty_binary() {
    let tmp = tempfile::tempdir().unwrap();
    touch_binary(tmp.path(), "debug", "unnamed");

    let summary = summarize_server("unnamed", &internal("", true), Some(tmp.path()));
    assert_eq!(summary.server_type, "internal");
    assert_eq!(summary.status.as_deref(), Some("debug-only"));
    assert!(summary.binary_debug.unwrap().ends_with("unnamed"));
    assert!(summary.binary_release.is_none());
}

#[test]
fn internal_summary_reports_ready_when_both_builds_exist() {
    let tmp = tempfile::tempdir().unwrap();
    touch_binary(tmp.path(), "debug", "svc-bin");
    touch_binary(tmp.path(), "release", "svc-bin");

    let summary = summarize_server("svc", &internal("svc-bin", true), Some(tmp.path()));
    assert_eq!(summary.status.as_deref(), Some("ready"));
    assert_eq!(summary.port, 5010);
    assert!(summary.debug_created_at.is_some());
    assert!(summary.release_created_at.is_some());
}
