use std::collections::HashSet;
use systemprompt_identifiers::{ExecutionStepId, LogId, TokenId, ArtifactId, DbValue, ToDbValue};

#[test]
fn execution_step_id_generate_uuid_format() {
    let id = ExecutionStepId::generate();
    assert_eq!(id.as_str().len(), 36);
    assert_eq!(id.as_str().chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn execution_step_id_generate_unique() {
    let ids: HashSet<String> = (0..10).map(|_| ExecutionStepId::generate().as_str().to_string()).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn execution_step_id_serde_transparent() {
    let id = ExecutionStepId::new("step-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"step-1\"");
    let deserialized: ExecutionStepId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn log_id_generate_uuid_format() {
    let id = LogId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn log_id_generate_unique() {
    let id1 = LogId::generate();
    let id2 = LogId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn token_id_generate_uuid_format() {
    let id = TokenId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn token_id_generate_unique() {
    let id1 = TokenId::generate();
    let id2 = TokenId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn artifact_id_generate_uuid_format() {
    let id = ArtifactId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn artifact_id_generate_unique() {
    let id1 = ArtifactId::generate();
    let id2 = ArtifactId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn all_execution_ids_serde_transparent() {
    let step = ExecutionStepId::new("s");
    assert_eq!(serde_json::to_string(&step).unwrap(), "\"s\"");
    let log = LogId::new("l");
    assert_eq!(serde_json::to_string(&log).unwrap(), "\"l\"");
    let token = TokenId::new("t");
    assert_eq!(serde_json::to_string(&token).unwrap(), "\"t\"");
    let artifact = ArtifactId::new("a");
    assert_eq!(serde_json::to_string(&artifact).unwrap(), "\"a\"");
}

#[test]
fn all_execution_ids_to_db_value() {
    assert!(matches!(ExecutionStepId::new("s").to_db_value(), DbValue::String(ref v) if v == "s"));
    assert!(matches!(LogId::new("l").to_db_value(), DbValue::String(ref v) if v == "l"));
    assert!(matches!(TokenId::new("t").to_db_value(), DbValue::String(ref v) if v == "t"));
    assert!(matches!(ArtifactId::new("a").to_db_value(), DbValue::String(ref v) if v == "a"));
}

#[test]
fn all_execution_ids_into_string() {
    let s: String = ExecutionStepId::new("step").into();
    assert_eq!(s, "step");
    let s: String = LogId::new("log").into();
    assert_eq!(s, "log");
    let s: String = TokenId::new("tok").into();
    assert_eq!(s, "tok");
    let s: String = ArtifactId::new("art").into();
    assert_eq!(s, "art");
}

#[test]
fn all_execution_ids_from_str_and_string_equal() {
    let a: ExecutionStepId = "x".into();
    let b: ExecutionStepId = String::from("x").into();
    assert_eq!(a, b);
}
