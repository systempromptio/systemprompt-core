//! Unit tests for TraceId type.

use std::collections::HashSet;
use systemprompt_identifiers::{TraceId, ToDbValue, DbValue};

#[test]
fn test_trace_id_new() {
    let id = TraceId::new("trace-123");
    assert_eq!(id.as_str(), "trace-123");
}

#[test]
fn test_trace_id_system() {
    let id = TraceId::system();
    assert_eq!(id.as_str(), "system");
}

#[test]
fn test_trace_id_generate() {
    let id = TraceId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_trace_id_generate_unique() {
    let id1 = TraceId::generate();
    let id2 = TraceId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_trace_id_display() {
    let id = TraceId::new("display-trace");
    assert_eq!(format!("{}", id), "display-trace");
}

#[test]
fn test_trace_id_from_string() {
    let id: TraceId = String::from("from-string-trace").into();
    assert_eq!(id.as_str(), "from-string-trace");
}

#[test]
fn test_trace_id_from_str() {
    let id: TraceId = "from-str-trace".into();
    assert_eq!(id.as_str(), "from-str-trace");
}

#[test]
fn test_trace_id_as_ref() {
    let id = TraceId::new("as-ref-trace");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-trace");
}

#[test]
fn test_trace_id_clone_and_eq() {
    let id1 = TraceId::new("clone-trace");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_trace_id_hash() {
    let id1 = TraceId::new("hash-trace");
    let id2 = TraceId::new("hash-trace");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_trace_id_serialize_json() {
    let id = TraceId::new("serialize-trace");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-trace\"");
}

#[test]
fn test_trace_id_deserialize_json() {
    let id: TraceId = serde_json::from_str("\"deserialize-trace\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-trace");
}

#[test]
fn test_trace_id_to_db_value() {
    let id = TraceId::new("db-value-trace");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-trace"));
}

#[test]
fn test_trace_id_ref_to_db_value() {
    let id = TraceId::new("db-value-ref-trace");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-trace"));
}

#[test]
fn test_trace_id_debug() {
    let id = TraceId::new("debug-trace");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("TraceId"));
    assert!(debug_str.contains("debug-trace"));
}

#[test]
fn test_trace_id_empty_allowed() {
    let id = TraceId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_trace_id_uuid_format() {
    let id = TraceId::new("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
}

#[test]
fn test_trace_id_generated_not_system() {
    let id = TraceId::generate();
    assert_ne!(id.as_str(), "system");
}
