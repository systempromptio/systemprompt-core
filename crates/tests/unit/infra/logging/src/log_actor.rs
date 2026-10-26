//! Tests for LogActor construction and field access.

use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::LogActor;
use systemprompt_test_fixtures::fixture_system_admin;

fn uid() -> UserId {
    UserId::new("actor-user")
}

fn sid() -> SessionId {
    SessionId::new("actor-session")
}

fn tid() -> TraceId {
    TraceId::new("actor-trace")
}

#[test]
fn log_actor_new_stores_fields() {
    let actor = LogActor::new(uid(), sid(), tid());
    assert_eq!(actor.user_id, uid());
    assert_eq!(actor.session_id, sid());
    assert_eq!(actor.trace_id, tid());
}

#[test]
fn log_actor_debug_includes_fields() {
    let actor = LogActor::new(uid(), sid(), tid());
    let debug = format!("{actor:?}");
    assert!(debug.contains("LogActor"));
    assert!(debug.contains("actor-user"));
    assert!(debug.contains("actor-session"));
    assert!(debug.contains("actor-trace"));
}


#[test]
fn log_actor_platform_attributes_to_the_given_owner() {
    let owner = fixture_system_admin("platform-actor-test");
    let trace = TraceId::new("platform-trace");
    let actor = LogActor::platform(&owner, trace.clone());
    assert_eq!(actor.trace_id, trace);
    assert_eq!(actor.session_id, SessionId::system());
    assert_eq!(&actor.user_id, owner.id());
}
