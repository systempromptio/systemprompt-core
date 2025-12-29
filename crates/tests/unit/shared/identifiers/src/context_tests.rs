//! Unit tests for ContextId type.

use std::collections::HashSet;
use systemprompt_identifiers::{ContextId, ToDbValue, DbValue};

#[test]
fn test_context_id_new() {
    let id = ContextId::new("context-123");
    assert_eq!(id.as_str(), "context-123");
}

#[test]
fn test_context_id_system() {
    let id = ContextId::system();
    assert_eq!(id.as_str(), "system");
}

#[test]
fn test_context_id_generate() {
    let id = ContextId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_context_id_generate_unique() {
    let id1 = ContextId::generate();
    let id2 = ContextId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_context_id_is_system() {
    let system_id = ContextId::system();
    let other_id = ContextId::new("other");

    assert!(system_id.is_system());
    assert!(!other_id.is_system());
}

#[test]
fn test_context_id_is_anonymous() {
    let anon_id = ContextId::new("anonymous");
    let other_id = ContextId::new("other");

    assert!(anon_id.is_anonymous());
    assert!(!other_id.is_anonymous());
}

#[test]
fn test_context_id_display() {
    let id = ContextId::new("display-context");
    assert_eq!(format!("{}", id), "display-context");
}

#[test]
fn test_context_id_from_string() {
    let id: ContextId = String::from("from-string-context").into();
    assert_eq!(id.as_str(), "from-string-context");
}

#[test]
fn test_context_id_from_str() {
    let id: ContextId = "from-str-context".into();
    assert_eq!(id.as_str(), "from-str-context");
}

#[test]
fn test_context_id_as_ref() {
    let id = ContextId::new("as-ref-context");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-context");
}

#[test]
fn test_context_id_clone_and_eq() {
    let id1 = ContextId::new("clone-context");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_context_id_hash() {
    let id1 = ContextId::new("hash-context");
    let id2 = ContextId::new("hash-context");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_context_id_serialize_json() {
    let id = ContextId::new("serialize-context");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-context\"");
}

#[test]
fn test_context_id_deserialize_json() {
    let id: ContextId = serde_json::from_str("\"deserialize-context\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-context");
}

#[test]
fn test_context_id_to_db_value() {
    let id = ContextId::new("db-value-context");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-context"));
}

#[test]
fn test_context_id_ref_to_db_value() {
    let id = ContextId::new("db-value-ref-context");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-context"));
}

#[test]
fn test_context_id_debug() {
    let id = ContextId::new("debug-context");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("ContextId"));
    assert!(debug_str.contains("debug-context"));
}

#[test]
fn test_context_id_system_is_not_anonymous() {
    let id = ContextId::system();
    assert!(!id.is_anonymous());
}

#[test]
fn test_context_id_generated_is_not_system() {
    let id = ContextId::generate();
    assert!(!id.is_system());
}

#[test]
fn test_context_id_generated_is_not_anonymous() {
    let id = ContextId::generate();
    assert!(!id.is_anonymous());
}
