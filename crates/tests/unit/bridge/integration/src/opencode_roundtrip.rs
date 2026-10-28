//! The `OpenCode` config round-trip: a written policy or provider the client
//! dropped fails doctor, named by file and entry; order is part of a policy's
//! meaning, so a reordered loaded set is a loss too.

use std::path::Path;

use serde_json::json;
use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::opencode::check_round_trip;
use systemprompt_bridge::integration::opencode::{
    ClientProbeError, Loss, RoundTrip, compare, written_config,
};

const SOURCE: &str = "/etc/opencode/opencode.json";

fn written() -> serde_json::Value {
    json!({
        "providers": { "systemprompt": { "package": "x" } },
        "policies": [
            { "effect": "deny", "action": "tool.use", "resource": "shell:*" },
            { "effect": "allow", "action": "tool.use", "resource": "shell:ls *" }
        ]
    })
}

fn round_trip(written: &serde_json::Value, loaded: &serde_json::Value) -> RoundTrip {
    compare(&[written_config(Path::new(SOURCE), written)], loaded)
}

#[test]
fn a_faithful_load_is_kept() {
    let loaded = json!({
        "providers": { "systemprompt": {}, "other": {} },
        "policies": [
            { "effect": "deny", "action": "provider.use", "resource": "*" },
            { "effect": "deny", "action": "tool.use", "resource": "shell:*" },
            { "effect": "allow", "action": "tool.use", "resource": "shell:ls *" }
        ]
    });
    assert!(matches!(round_trip(&written(), &loaded), RoundTrip::Kept));
    let check = check_round_trip(&RoundTrip::Kept).expect("reported");
    assert_eq!(check.status, Status::Ok);
}

#[test]
fn a_typoed_deny_dropped_on_load_fails_by_file_and_entry() {
    let mut doc = written();
    doc["policies"][0]["effect"] = json!("denny");
    let loaded = json!({
        "providers": { "systemprompt": {} },
        "policies": [
            { "effect": "allow", "action": "tool.use", "resource": "shell:ls *" }
        ]
    });
    let RoundTrip::Lost(lost) = round_trip(&doc, &loaded) else {
        panic!("a dropped policy is a loss");
    };
    assert_eq!(lost.len(), 1);
    assert!(matches!(&lost[0], Loss::Policy { index: 0, .. }));
    let check = check_round_trip(&RoundTrip::Lost(lost)).expect("reported");
    assert_eq!(check.status, Status::Fail);
    assert!(check.detail.contains("policies[0]"), "{}", check.detail);
    assert!(check.detail.contains(SOURCE), "{}", check.detail);
    assert!(check.detail.contains("denny"), "{}", check.detail);
}

#[test]
fn a_reordered_load_loses_the_later_policy() {
    let loaded = json!({
        "providers": { "systemprompt": {} },
        "policies": [
            { "effect": "allow", "action": "tool.use", "resource": "shell:ls *" },
            { "effect": "deny", "action": "tool.use", "resource": "shell:*" }
        ]
    });
    let RoundTrip::Lost(lost) = round_trip(&written(), &loaded) else {
        panic!("an allow now shadowed by its deny is a loss");
    };
    assert!(matches!(&lost[..], [Loss::Policy { index: 1, .. }]));
}

#[test]
fn a_missing_provider_is_a_loss() {
    let loaded = json!({ "policies": written()["policies"].clone() });
    let RoundTrip::Lost(lost) = round_trip(&written(), &loaded) else {
        panic!("a dropped provider is a loss");
    };
    assert!(matches!(&lost[..], [Loss::Provider { .. }]));
}

#[test]
fn nothing_written_or_no_client_reports_nothing() {
    assert!(check_round_trip(&RoundTrip::NothingWritten).is_none());
    assert!(check_round_trip(&RoundTrip::NotInstalled).is_none());
    let unreadable =
        check_round_trip(&RoundTrip::Unreadable(ClientProbeError::TimedOut)).expect("reported");
    assert_eq!(unreadable.status, Status::Warn);
}
