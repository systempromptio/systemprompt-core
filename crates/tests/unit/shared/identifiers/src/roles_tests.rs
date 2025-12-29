//! Unit tests for RoleId type.

use std::collections::HashSet;
use systemprompt_identifiers::{RoleId, ToDbValue, DbValue};

#[test]
fn test_role_id_new() {
    let id = RoleId::new("role-123");
    assert_eq!(id.as_str(), "role-123");
}

#[test]
fn test_role_id_display() {
    let id = RoleId::new("display-role");
    assert_eq!(format!("{}", id), "display-role");
}

#[test]
fn test_role_id_from_string() {
    let id: RoleId = String::from("from-string-role").into();
    assert_eq!(id.as_str(), "from-string-role");
}

#[test]
fn test_role_id_from_str() {
    let id: RoleId = "from-str-role".into();
    assert_eq!(id.as_str(), "from-str-role");
}

#[test]
fn test_role_id_as_ref() {
    let id = RoleId::new("as-ref-role");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-role");
}

#[test]
fn test_role_id_clone_and_eq() {
    let id1 = RoleId::new("clone-role");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_role_id_hash() {
    let id1 = RoleId::new("hash-role");
    let id2 = RoleId::new("hash-role");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_role_id_serialize_json() {
    let id = RoleId::new("serialize-role");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-role\"");
}

#[test]
fn test_role_id_deserialize_json() {
    let id: RoleId = serde_json::from_str("\"deserialize-role\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-role");
}

#[test]
fn test_role_id_to_db_value() {
    let id = RoleId::new("db-value-role");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-role"));
}

#[test]
fn test_role_id_ref_to_db_value() {
    let id = RoleId::new("db-value-ref-role");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-role"));
}

#[test]
fn test_role_id_debug() {
    let id = RoleId::new("debug-role");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("RoleId"));
    assert!(debug_str.contains("debug-role"));
}

#[test]
fn test_role_id_empty_allowed() {
    let id = RoleId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_role_id_admin() {
    let id = RoleId::new("admin");
    assert_eq!(id.as_str(), "admin");
}

#[test]
fn test_role_id_user() {
    let id = RoleId::new("user");
    assert_eq!(id.as_str(), "user");
}

#[test]
fn test_role_id_guest() {
    let id = RoleId::new("guest");
    assert_eq!(id.as_str(), "guest");
}
