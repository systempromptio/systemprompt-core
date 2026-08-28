use serde_json::json;
use systemprompt_bridge::sync::{HostFailure, SyncSummary};

fn summary() -> SyncSummary {
    SyncSummary {
        identity: "alice@example.com".into(),
        manifest_version: "41".into(),
        plugin_count: 2,
        skill_count: 3,
        agent_count: 1,
        hook_count: 4,
        mcp_count: 5,
        installed: vec!["governance-pack".into()],
        updated: vec!["review-standards".into()],
        removed: vec![],
        malformed: vec!["broken-plugin".into()],
        host_failures: vec![HostFailure {
            host_id: "claude-desktop".into(),
            error: "profile write denied by policy".into(),
        }],
        diagnostics: vec!["a skill is missing from every plugin's skills.include".into()],
    }
}

// The whole point of serialising the summary is that failures stop being a
// substring of `one_line()` and become addressable rows.
#[test]
fn host_failures_survive_as_structured_rows() {
    let value = serde_json::to_value(summary()).expect("summary serialises");

    assert_eq!(value["host_failures"][0]["host_id"], json!("claude-desktop"));
    assert_eq!(
        value["host_failures"][0]["error"],
        json!("profile write denied by policy")
    );
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
