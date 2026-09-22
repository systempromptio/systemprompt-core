//! A successful hit on a timer-driven bridge route never earns a `logs` row;
//! a failure on the same route, and any hit on a user-driven route, does.

use systemprompt_api::routes::gateway::access_log::persists_access_record;

#[test]
fn successful_polls_are_not_persisted() {
    for path in [
        "/v1/bridge/profile",
        "/v1/bridge/profile/usage",
        "/v1/bridge/heartbeat",
        "/v1/bridge/latest",
        "/v1/bridge/manifest",
    ] {
        assert!(!persists_access_record(path, 200), "{path} 200 persisted");
        assert!(!persists_access_record(path, 304), "{path} 304 persisted");
    }
}

#[test]
fn failed_polls_are_persisted() {
    assert!(persists_access_record("/v1/bridge/heartbeat", 401));
    assert!(persists_access_record("/v1/bridge/latest", 502));
    assert!(persists_access_record("/v1/bridge/profile", 429));
}

#[test]
fn user_driven_routes_always_persist() {
    assert!(persists_access_record("/v1/messages", 200));
    assert!(persists_access_record("/v1/chat/completions", 200));
    assert!(persists_access_record("/v1/bridge/stream", 200));
    assert!(persists_access_record(
        "/v1/bridge/plugins/p/skills/s/reference",
        200
    ));
    assert!(persists_access_record(
        "/v1/bridge/profile/enabled_hosts",
        200
    ));
}
