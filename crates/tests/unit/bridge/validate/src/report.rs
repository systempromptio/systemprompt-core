use std::fs;

use systemprompt_bridge::validate::{
    CheckLevel, CheckLine, ValidationReport, count_installed_plugins, summarise_last_sync,
};

fn line(level: CheckLevel, label: &str, value: &str) -> CheckLine {
    CheckLine {
        level,
        label: label.into(),
        value: value.into(),
    }
}

#[test]
fn rendered_includes_header_each_level_and_ok_result() {
    let report = ValidationReport {
        lines: vec![
            line(CheckLevel::Ok, "ok-label", "ok-value"),
            line(CheckLevel::Warn, "warn-label", "warn-value"),
            line(CheckLevel::Fail, "fail-label", "fail-value"),
            line(CheckLevel::Info, "info-label", "info-value"),
        ],
        any_failed: false,
    };

    let out = report.rendered();

    assert!(
        out.starts_with("systemprompt-bridge validate\n"),
        "expected header at start, got: {out}"
    );

    assert!(out.contains("  [ok]   ok-label: ok-value"));
    assert!(out.contains("  [warn] warn-label: warn-value"));
    assert!(out.contains("  [fail] fail-label: fail-value"));
    assert!(out.contains("         info-label: info-value"));

    assert!(
        out.trim_end().ends_with("Result: OK"),
        "expected OK result at end, got: {out}"
    );
}

#[test]
fn rendered_reports_fail_when_any_failed() {
    let report = ValidationReport {
        lines: vec![line(CheckLevel::Fail, "fail-label", "fail-value")],
        any_failed: true,
    };

    let out = report.rendered();

    assert!(out.starts_with("systemprompt-bridge validate\n"));
    assert!(
        out.contains("Result: FAIL"),
        "expected FAIL result, got: {out}"
    );
    assert!(!out.contains("Result: OK"));
}

#[test]
fn rendered_with_no_lines_is_just_header_and_result() {
    let report = ValidationReport {
        lines: Vec::new(),
        any_failed: false,
    };

    let out = report.rendered();

    assert_eq!(out, "systemprompt-bridge validate\n\nResult: OK\n");
}

#[test]
fn check_line_literal_construction_has_public_fields() {
    let l = CheckLine {
        level: CheckLevel::Ok,
        label: "foo".into(),
        value: "bar".into(),
    };

    assert_eq!(l.level, CheckLevel::Ok);
    assert_eq!(l.label, "foo");
    assert_eq!(l.value, "bar");
}

#[test]
fn summarise_last_sync_full_record() {
    let raw = r#"{
        "synced_at": "2026-06-03T12:00:00Z",
        "manifest_version": "2026-06-03T12:00:00Z-cafecafe",
        "mcp_server_count": 4
    }"#;

    assert_eq!(
        summarise_last_sync(raw),
        "2026-06-03T12:00:00Z (manifest 2026-06-03T12:00:00Z-cafecafe, 4 MCP server(s))"
    );
}

#[test]
fn summarise_last_sync_missing_fields_uses_defaults() {
    let raw = "{}";

    assert_eq!(
        summarise_last_sync(raw),
        "unknown (manifest ?, 0 MCP server(s))"
    );
}

#[test]
fn summarise_last_sync_partial_record() {
    let raw = r#"{ "manifest_version": "2026-06-03T12:00:00Z-deadbeef" }"#;

    assert_eq!(
        summarise_last_sync(raw),
        "unknown (manifest 2026-06-03T12:00:00Z-deadbeef, 0 MCP server(s))"
    );
}

#[test]
fn summarise_last_sync_partial_sync_names_the_failed_hosts() {
    let raw = r#"{
        "synced_at": "2026-09-17T11:05:24Z",
        "manifest_version": "2026-09-17T11:04:01Z-000001a0af09abc9",
        "mcp_server_count": 2,
        "host_failures": ["claude-code: apply: io error in remove managed MCP policy: EOF while parsing a value"]
    }"#;

    assert_eq!(
        summarise_last_sync(raw),
        "partial — 1 host(s) failed (claude-code); 2026-09-17T11:05:24Z (last applied manifest 2026-09-17T11:04:01Z-000001a0af09abc9, 2 MCP server(s))"
    );
}

#[test]
fn summarise_last_sync_partial_sync_counts_malformed_plugins() {
    let raw = r#"{
        "synced_at": "2026-09-17T11:05:24Z",
        "host_failures": ["claude-code: apply: boom", "opencode: apply: boom"],
        "malformed_plugins": ["astound-dev"]
    }"#;

    assert_eq!(
        summarise_last_sync(raw),
        "partial — 2 host(s) failed (claude-code, opencode), 1 malformed plugin(s); 2026-09-17T11:05:24Z (last applied manifest ?, 0 MCP server(s))"
    );
}

#[test]
fn summarise_last_sync_invalid_json_is_unparseable() {
    assert_eq!(summarise_last_sync("not json"), "unparseable");
}

#[test]
fn count_installed_plugins_counts_subdirs_excluding_hidden_and_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    for name in ["alpha", "beta", "gamma"] {
        fs::create_dir(root.join(name)).expect("create plugin dir");
    }
    fs::create_dir(root.join(".hidden")).expect("create hidden dir");
    fs::write(root.join("regular.txt"), b"data").expect("write file");

    assert_eq!(count_installed_plugins(root), Some(3));
}

#[test]
fn count_installed_plugins_empty_dir_is_zero() {
    let dir = tempfile::tempdir().expect("tempdir");

    assert_eq!(count_installed_plugins(dir.path()), Some(0));
}

#[test]
fn count_installed_plugins_missing_path_is_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does-not-exist");

    assert_eq!(count_installed_plugins(&missing), None);
}

// A sentinel whose manifest version is not a real manifest version is a
// corrupt record, and the summary says so instead of inventing a value.
#[test]
fn summarise_last_sync_rejects_a_malformed_manifest_version() {
    assert_eq!(
        summarise_last_sync(r#"{ "manifest_version": "1.2.3" }"#),
        "unparseable"
    );
}
