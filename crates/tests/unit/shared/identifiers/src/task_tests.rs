//! Unit tests for TaskId type.

use std::collections::HashSet;
use systemprompt_identifiers::{TaskId, ToDbValue, DbValue};

#[test]
fn test_task_id_new() {
    let id = TaskId::new("task-123");
    assert_eq!(id.as_str(), "task-123");
}

#[test]
fn test_task_id_generate() {
    let id = TaskId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_task_id_generate_unique() {
    let id1 = TaskId::generate();
    let id2 = TaskId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_task_id_display() {
    let id = TaskId::new("display-task");
    assert_eq!(format!("{}", id), "display-task");
}

#[test]
fn test_task_id_from_string() {
    let id: TaskId = String::from("from-string-task").into();
    assert_eq!(id.as_str(), "from-string-task");
}

#[test]
fn test_task_id_from_str() {
    let id: TaskId = "from-str-task".into();
    assert_eq!(id.as_str(), "from-str-task");
}

#[test]
fn test_task_id_as_ref() {
    let id = TaskId::new("as-ref-task");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-task");
}

#[test]
fn test_task_id_clone_and_eq() {
    let id1 = TaskId::new("clone-task");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_task_id_hash() {
    let id1 = TaskId::new("hash-task");
    let id2 = TaskId::new("hash-task");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_task_id_serialize_json() {
    let id = TaskId::new("serialize-task");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-task\"");
}

#[test]
fn test_task_id_deserialize_json() {
    let id: TaskId = serde_json::from_str("\"deserialize-task\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-task");
}

#[test]
fn test_task_id_to_db_value() {
    let id = TaskId::new("db-value-task");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-task"));
}

#[test]
fn test_task_id_ref_to_db_value() {
    let id = TaskId::new("db-value-ref-task");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-task"));
}

#[test]
fn test_task_id_debug() {
    let id = TaskId::new("debug-task");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("TaskId"));
    assert!(debug_str.contains("debug-task"));
}

#[test]
fn test_task_id_empty_allowed() {
    let id = TaskId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_task_id_uuid_format() {
    let id = TaskId::new("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
}
