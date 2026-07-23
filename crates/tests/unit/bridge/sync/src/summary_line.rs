use systemprompt_bridge::sync::{HostFailure, SyncSummary, warn_unsafe_flags};

fn summary() -> SyncSummary {
    SyncSummary {
        identity: "alice@example.com".into(),
        manifest_version: "2026-05-01T12:00:00Z-deadbeef".into(),
        plugin_count: 2,
        skill_count: 3,
        agent_count: 1,
        hook_count: 4,
        mcp_count: 5,
        installed: vec!["a".into()],
        updated: vec!["b".into()],
        removed: vec![],
        malformed: vec![],
        host_failures: vec![],
    }
}

#[test]
fn a_clean_sync_renders_ok_with_every_count() {
    let line = summary().one_line();
    assert!(line.starts_with("sync ok (alice@example.com):"), "{line}");
    assert!(
        line.contains("2 plugins (1 new, 1 updated, 0 removed)"),
        "{line}"
    );
    assert!(
        line.contains("3 skills, 1 agents, 4 hooks, 5 MCP"),
        "{line}"
    );
    assert!(
        line.ends_with("manifest 2026-05-01T12:00:00Z-deadbeef"),
        "{line}"
    );
}

#[test]
fn malformed_plugins_are_named_in_a_warning_suffix() {
    let mut s = summary();
    s.malformed = vec!["ghost".into(), "husk".into()];
    let line = s.one_line();
    assert!(
        line.starts_with("sync ok"),
        "malformed is a warning, not a failure: {line}"
    );
    assert!(
        line.contains(
            "WARNING: 2 malformed plugin(s) missing claude-plugin/plugin.json: ghost, husk"
        ),
        "{line}"
    );
}

#[test]
fn a_failing_host_downgrades_the_line_to_partial_and_keeps_one_error_line() {
    let mut s = summary();
    s.host_failures = vec![HostFailure {
        host_id: "cowork".into(),
        error: "first line of the error\nsecond line that must not appear".into(),
    }];
    let line = s.one_line();
    assert!(line.starts_with("sync PARTIAL"), "{line}");
    assert!(
        line.contains("1 host(s) failed: cowork (first line of the error)"),
        "{line}"
    );
    assert!(
        !line.contains("second line"),
        "only the first error line belongs on the summary line: {line}"
    );
    assert!(line.ends_with("see bridge.log"), "{line}");
}

#[test]
fn several_failing_hosts_are_joined() {
    let mut s = summary();
    s.host_failures = vec![
        HostFailure {
            host_id: "cowork".into(),
            error: "no session".into(),
        },
        HostFailure {
            host_id: "codex-cli".into(),
            error: "permission denied".into(),
        },
    ];
    let line = s.one_line();
    assert!(
        line.contains("2 host(s) failed: cowork (no session); codex-cli (permission denied)"),
        "{line}"
    );
}

#[test]
fn the_unsafe_flag_warnings_run_without_a_pinned_pubkey() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let root = dir.path().display().to_string();
    let pinned = temp_env::with_vars(
        vec![
            ("XDG_CONFIG_HOME", Some(root.clone())),
            ("HOME", Some(root)),
            ("SP_BRIDGE_CONFIG", None),
        ],
        || {
            warn_unsafe_flags(true, true, true);
            warn_unsafe_flags(false, false, false);
            systemprompt_bridge::config::pinned_pubkey()
        },
    );
    assert!(
        pinned.is_none(),
        "the tofu warning path is the one exercised when nothing is pinned"
    );
}
