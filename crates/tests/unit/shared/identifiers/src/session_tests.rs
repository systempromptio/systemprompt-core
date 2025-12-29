//! Unit tests for SessionId type.

use std::collections::HashSet;
use systemprompt_identifiers::{SessionId, ToDbValue, DbValue};

#[test]
fn test_session_id_new() {
    let id = SessionId::new("session-123");
    assert_eq!(id.as_str(), "session-123");
}

#[test]
fn test_session_id_system() {
    let id = SessionId::system();
    assert_eq!(id.as_str(), "system");
}

#[test]
fn test_session_id_display() {
    let id = SessionId::new("display-session");
    assert_eq!(format!("{}", id), "display-session");
}

#[test]
fn test_session_id_from_string() {
    let id: SessionId = String::from("from-string-session").into();
    assert_eq!(id.as_str(), "from-string-session");
}

#[test]
fn test_session_id_from_str() {
    let id: SessionId = "from-str-session".into();
    assert_eq!(id.as_str(), "from-str-session");
}

#[test]
fn test_session_id_as_ref() {
    let id = SessionId::new("as-ref-session");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-session");
}

#[test]
fn test_session_id_clone_and_eq() {
    let id1 = SessionId::new("clone-session");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_session_id_hash() {
    let id1 = SessionId::new("hash-session");
    let id2 = SessionId::new("hash-session");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_session_id_serialize_json() {
    let id = SessionId::new("serialize-session");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-session\"");
}

#[test]
fn test_session_id_deserialize_json() {
    let id: SessionId = serde_json::from_str("\"deserialize-session\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-session");
}

#[test]
fn test_session_id_to_db_value() {
    let id = SessionId::new("db-value-session");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-session"));
}

#[test]
fn test_session_id_ref_to_db_value() {
    let id = SessionId::new("db-value-ref-session");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-session"));
}

#[test]
fn test_session_id_debug() {
    let id = SessionId::new("debug-session");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("SessionId"));
    assert!(debug_str.contains("debug-session"));
}

#[test]
fn test_session_id_empty_allowed() {
    let id = SessionId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_session_id_uuid_format() {
    let id = SessionId::new("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
}
