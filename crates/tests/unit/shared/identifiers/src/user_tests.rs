//! Unit tests for UserId type.

use std::collections::HashSet;
use systemprompt_identifiers::{UserId, ToDbValue, DbValue};

#[test]
fn test_user_id_new() {
    let id = UserId::new("user-123");
    assert_eq!(id.as_str(), "user-123");
}

#[test]
fn test_user_id_anonymous() {
    let id = UserId::anonymous();
    assert_eq!(id.as_str(), "anonymous");
}

#[test]
fn test_user_id_system() {
    let id = UserId::system();
    assert_eq!(id.as_str(), "system");
}

#[test]
fn test_user_id_is_system() {
    let system_id = UserId::system();
    let other_id = UserId::new("other");

    assert!(system_id.is_system());
    assert!(!other_id.is_system());
}

#[test]
fn test_user_id_is_anonymous() {
    let anon_id = UserId::anonymous();
    let other_id = UserId::new("other");

    assert!(anon_id.is_anonymous());
    assert!(!other_id.is_anonymous());
}

#[test]
fn test_user_id_display() {
    let id = UserId::new("display-user");
    assert_eq!(format!("{}", id), "display-user");
}

#[test]
fn test_user_id_from_string() {
    let id: UserId = String::from("from-string-user").into();
    assert_eq!(id.as_str(), "from-string-user");
}

#[test]
fn test_user_id_from_str() {
    let id: UserId = "from-str-user".into();
    assert_eq!(id.as_str(), "from-str-user");
}

#[test]
fn test_user_id_as_ref() {
    let id = UserId::new("as-ref-user");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-user");
}

#[test]
fn test_user_id_clone_and_eq() {
    let id1 = UserId::new("clone-user");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_user_id_hash() {
    let id1 = UserId::new("hash-user");
    let id2 = UserId::new("hash-user");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_user_id_serialize_json() {
    let id = UserId::new("serialize-user");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-user\"");
}

#[test]
fn test_user_id_deserialize_json() {
    let id: UserId = serde_json::from_str("\"deserialize-user\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-user");
}

#[test]
fn test_user_id_to_db_value() {
    let id = UserId::new("db-value-user");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-user"));
}

#[test]
fn test_user_id_ref_to_db_value() {
    let id = UserId::new("db-value-ref-user");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-user"));
}

#[test]
fn test_user_id_debug() {
    let id = UserId::new("debug-user");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("UserId"));
    assert!(debug_str.contains("debug-user"));
}

#[test]
fn test_user_id_empty_allowed() {
    let id = UserId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_user_id_system_is_not_anonymous() {
    let id = UserId::system();
    assert!(!id.is_anonymous());
}

#[test]
fn test_user_id_anonymous_is_not_system() {
    let id = UserId::anonymous();
    assert!(!id.is_system());
}

#[test]
fn test_user_id_uuid_format() {
    let id = UserId::new("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
}

#[test]
fn test_user_id_email_format() {
    let id = UserId::new("user@example.com");
    assert_eq!(id.as_str(), "user@example.com");
}
