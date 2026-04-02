use std::collections::HashSet;
use systemprompt_identifiers::{ContextId, DbValue, ToDbValue};

#[test]
fn system_factory_value() {
    assert_eq!(ContextId::system().as_str(), "system");
}

#[test]
fn empty_factory_produces_empty_string() {
    let id = ContextId::empty();
    assert!(id.is_empty());
    assert_eq!(id.as_str(), "");
}

#[test]
fn is_empty_false_for_non_empty() {
    assert!(!ContextId::new("something").is_empty());
    assert!(!ContextId::system().is_empty());
}

#[test]
fn is_system_true_only_for_system() {
    assert!(ContextId::system().is_system());
    assert!(!ContextId::new("other").is_system());
    assert!(!ContextId::empty().is_system());
}

#[test]
fn is_anonymous_true_only_for_anonymous() {
    assert!(ContextId::new("anonymous").is_anonymous());
    assert!(!ContextId::system().is_anonymous());
    assert!(!ContextId::new("other").is_anonymous());
}

#[test]
fn generate_produces_uuid_format() {
    let id = ContextId::generate();
    assert_eq!(id.as_str().len(), 36);
    assert!(!id.is_empty());
    assert!(!id.is_system());
    assert!(!id.is_anonymous());
}

#[test]
fn generate_unique_across_calls() {
    let ids: HashSet<String> = (0..50).map(|_| ContextId::generate().as_str().to_string()).collect();
    assert_eq!(ids.len(), 50);
}

#[test]
fn display_format() {
    let id = ContextId::new("ctx-42");
    assert_eq!(format!("{}", id), "ctx-42");
}

#[test]
fn serde_transparent_json() {
    let id = ContextId::new("serde-ctx");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serde-ctx\"");
    let deserialized: ContextId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn from_string_and_str_produce_equal() {
    let from_str: ContextId = "test".into();
    let from_string: ContextId = String::from("test").into();
    assert_eq!(from_str, from_string);
}

#[test]
fn to_db_value_owned_and_ref() {
    let id = ContextId::new("db");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "db"));
}
