//! Tests for LogActor construction and field access.

use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, install_log_attribution};
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
fn log_actor_clone_equals_original() {
    let actor = LogActor::new(uid(), sid(), tid());
    let cloned = actor.clone();
    assert_eq!(cloned.user_id, actor.user_id);
    assert_eq!(cloned.session_id, actor.session_id);
    assert_eq!(cloned.trace_id, actor.trace_id);
}

#[test]
fn log_actor_platform_requires_installed_attribution() {
    install_log_attribution(fixture_system_admin("platform-actor-test"));
    let trace = TraceId::new("platform-trace");
    let actor = LogActor::platform(trace.clone()).expect("attribution installed");
    assert_eq!(actor.trace_id, trace);
    assert_eq!(actor.session_id, SessionId::system());
    assert!(!actor.user_id.as_str().is_empty());
}
