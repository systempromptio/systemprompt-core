use serde_json::json;
use systemprompt_bridge::ids::HostId;
use systemprompt_bridge::sync::{HostFailure, HostWarning, HostWarningKind, SyncSummary};

fn summary() -> SyncSummary {
    SyncSummary {
        identity: "alice@example.com".into(),
        manifest_version: "41".into(),
        plugin_count: 2,
        skill_count: 3,
        rule_count: 0,
        agent_count: 1,
        hook_count: 4,
        mcp_count: 5,
        artifact_count: 7,
        installed: vec!["governance-pack".into()],
        updated: vec!["review-standards".into()],
        removed: vec![],
        malformed: vec!["broken-plugin".into()],
        host_failures: vec![HostFailure {
            host_id: HostId::new("claude-desktop"),
            emitter: "claude-desktop".to_owned(),
            error: "profile write denied by policy".into(),
            needs_elevation: false,
        }],
        host_warnings: Vec::new(),
        diagnostics: vec!["a skill is missing from every plugin's skills.include".into()],
    }
}

// The whole point of serialising the summary is that failures stop being a
// substring of `one_line()` and become addressable rows.
#[test]
fn host_failures_survive_as_structured_rows() {
    let value = serde_json::to_value(summary()).expect("summary serialises");

    assert_eq!(
        value["host_failures"][0]["host_id"],
        json!("claude-desktop")
    );
    assert_eq!(
        value["host_failures"][0]["error"],
        json!("profile write denied by policy")
    );
    assert_eq!(value["host_failures"][0]["needs_elevation"], json!(false));
}

// Why: the front end offers "Repair as administrator" only for a failure the
// sync classified as elevation-required; the flag must reach it as a field.
#[test]
fn an_elevation_required_failure_carries_its_flag_on_the_wire() {
    let mut s = summary();
    s.host_failures[0].needs_elevation = true;
    let value = serde_json::to_value(s).expect("summary serialises");
    assert_eq!(value["host_failures"][0]["needs_elevation"], json!(true));
}

#[test]
fn the_change_lists_and_counts_both_cross_the_boundary() {
    let value = serde_json::to_value(summary()).expect("summary serialises");

    assert_eq!(value["installed"], json!(["governance-pack"]));
    assert_eq!(value["updated"], json!(["review-standards"]));
    assert_eq!(value["removed"], json!([]));
    assert_eq!(value["malformed"], json!(["broken-plugin"]));
    assert_eq!(value["plugin_count"], json!(2));
    assert_eq!(value["skill_count"], json!(3));
    assert_eq!(value["manifest_version"], json!("41"));
}

#[test]
fn diagnostics_are_carried_verbatim() {
    let value = serde_json::to_value(summary()).expect("summary serialises");

    assert_eq!(
        value["diagnostics"][0],
        json!("a skill is missing from every plugin's skills.include")
    );
}

fn warning(kind: HostWarningKind, host: &str) -> HostWarning {
    HostWarning {
        kind,
        host_id: HostId::new(host),
        message: "detail".into(),
    }
}

// Why: the GUI's 30 s tick re-requests a sync when Claude Desktop had not
// opened Cowork yet. It must key on that outcome alone: the feedback-timeout
// warning under the same host re-armed the tick on every cycle for the life
// of the window (one sync every 30 s, each ~110 s long).
#[test]
fn cowork_enable_is_deferred_only_by_the_session_missing_warning() {
    let desktop = HostId::new("claude-desktop");
    let mut s = summary();

    s.host_warnings = vec![warning(
        HostWarningKind::EvidenceUnacknowledged,
        "claude-desktop",
    )];
    assert!(
        !s.cowork_enable_deferred(&desktop),
        "evidence pending is not a deferral"
    );

    s.host_warnings = vec![warning(
        HostWarningKind::PluginDependencies,
        "claude-desktop",
    )];
    assert!(
        !s.cowork_enable_deferred(&desktop),
        "a dependency note is not a deferral"
    );

    s.host_warnings = vec![warning(
        HostWarningKind::CoworkSessionMissing,
        "claude-code",
    )];
    assert!(
        !s.cowork_enable_deferred(&desktop),
        "another host's warning is not this host's"
    );

    s.host_warnings = vec![
        warning(HostWarningKind::EvidenceUnacknowledged, "claude-desktop"),
        warning(HostWarningKind::CoworkSessionMissing, "claude-desktop"),
    ];
    assert!(s.cowork_enable_deferred(&desktop));

    s.host_warnings = Vec::new();
    assert!(
        !s.cowork_enable_deferred(&desktop),
        "a clean report clears the deferral"
    );
}

#[test]
fn a_host_warning_carries_its_kind_on_the_wire() {
    let mut s = summary();
    s.host_warnings = vec![warning(
        HostWarningKind::CoworkSessionMissing,
        "claude-desktop",
    )];
    let value = serde_json::to_value(&s).expect("summary serialises");
    assert_eq!(
        value["host_warnings"][0]["kind"],
        json!("cowork_session_missing")
    );
    assert_eq!(
        value["host_warnings"][0]["host_id"],
        json!("claude-desktop")
    );

    let back: HostWarning =
        serde_json::from_value(value["host_warnings"][0].clone()).expect("warning round-trips");
    assert_eq!(back.kind, HostWarningKind::CoworkSessionMissing);
}
