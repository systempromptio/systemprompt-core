use std::collections::HashSet;
use systemprompt_identifiers::{TaskId, DbValue, ToDbValue};

#[test]
fn generate_uuid_format() {
    let id = TaskId::generate();
    assert_eq!(id.as_str().len(), 36);
    assert_eq!(id.as_str().chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn generate_unique() {
    let ids: HashSet<String> = (0..20).map(|_| TaskId::generate().as_str().to_string()).collect();
    assert_eq!(ids.len(), 20);
}

#[test]
fn display_format() {
    let id = TaskId::new("task-42");
    assert_eq!(format!("{}", id), "task-42");
}

#[test]
fn serde_transparent_json() {
    let id = TaskId::new("task-serde");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"task-serde\"");
    let deserialized: TaskId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn from_str_and_string_equal() {
    let a: TaskId = "x".into();
    let b: TaskId = String::from("x").into();
    assert_eq!(a, b);
}

#[test]
fn into_string() {
    let s: String = TaskId::new("convert").into();
    assert_eq!(s, "convert");
}

#[test]
fn partial_eq_str() {
    let id = TaskId::new("cmp");
    assert!(id == "cmp");
    assert!("cmp" == id);
}

#[test]
fn to_db_value_owned_and_ref() {
    let id = TaskId::new("db");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "db"));
}
