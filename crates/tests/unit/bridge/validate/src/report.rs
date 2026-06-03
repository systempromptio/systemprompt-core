use std::fs;

use systemprompt_bridge::validate::{
    count_installed_plugins, summarise_last_sync, CheckLevel, CheckLine, ValidationReport,
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
        "manifest_version": "1.2.3",
        "mcp_server_count": 4
    }"#;

    assert_eq!(
        summarise_last_sync(raw),
        "2026-06-03T12:00:00Z (manifest 1.2.3, 4 MCP server(s))"
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
    let raw = r#"{ "manifest_version": "9.9.9" }"#;

    assert_eq!(
        summarise_last_sync(raw),
        "unknown (manifest 9.9.9, 0 MCP server(s))"
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
