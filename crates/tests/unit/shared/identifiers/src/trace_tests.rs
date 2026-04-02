use std::collections::HashSet;
use systemprompt_identifiers::{TraceId, DbValue, ToDbValue};

#[test]
fn system_factory_value() {
    assert_eq!(TraceId::system().as_str(), "system");
}

#[test]
fn generate_uuid_format() {
    let id = TraceId::generate();
    assert_eq!(id.as_str().len(), 36);
    assert_eq!(id.as_str().chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn generate_unique() {
    let ids: HashSet<String> = (0..20).map(|_| TraceId::generate().as_str().to_string()).collect();
    assert_eq!(ids.len(), 20);
}

#[test]
fn generated_is_not_system() {
    let id = TraceId::generate();
    assert_ne!(id.as_str(), "system");
}

#[test]
fn display_format() {
    let id = TraceId::new("trace-42");
    assert_eq!(format!("{}", id), "trace-42");
}

#[test]
fn serde_transparent_json() {
    let id = TraceId::new("trace-serde");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"trace-serde\"");
    let deserialized: TraceId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn from_string_and_str_produce_equal() {
    let a: TraceId = "x".into();
    let b: TraceId = String::from("x").into();
    assert_eq!(a, b);
}

#[test]
fn into_string() {
    let s: String = TraceId::new("convert").into();
    assert_eq!(s, "convert");
}

#[test]
fn to_db_value_owned_and_ref() {
    let id = TraceId::new("db");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "db"));
}
