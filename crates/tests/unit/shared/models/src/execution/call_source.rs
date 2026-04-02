use std::str::FromStr;
use systemprompt_models::execution::CallSource;

#[test]
fn call_source_from_str_agentic() {
    let source = CallSource::from_str("agentic").unwrap();
    assert_eq!(source, CallSource::Agentic);
}

#[test]
fn call_source_from_str_direct() {
    let source = CallSource::from_str("direct").unwrap();
    assert_eq!(source, CallSource::Direct);
}

#[test]
fn call_source_from_str_ephemeral() {
    let source = CallSource::from_str("ephemeral").unwrap();
    assert_eq!(source, CallSource::Ephemeral);
}

#[test]
fn call_source_from_str_case_insensitive() {
    let source = CallSource::from_str("AGENTIC").unwrap();
    assert_eq!(source, CallSource::Agentic);
}

#[test]
fn call_source_from_str_invalid() {
    let result = CallSource::from_str("invalid");
    assert!(result.is_err());
}

#[test]
fn call_source_as_str_agentic() {
    assert_eq!(CallSource::Agentic.as_str(), "agentic");
}

#[test]
fn call_source_as_str_direct() {
    assert_eq!(CallSource::Direct.as_str(), "direct");
}

#[test]
fn call_source_as_str_ephemeral() {
    assert_eq!(CallSource::Ephemeral.as_str(), "ephemeral");
}

#[test]
fn call_source_clone_equality() {
    let source = CallSource::Agentic;
    let cloned = source;
    assert_eq!(source, cloned);
}

#[test]
fn call_source_serde_roundtrip() {
    let source = CallSource::Direct;
    let json = serde_json::to_string(&source).unwrap();
    let deserialized: CallSource = serde_json::from_str(&json).unwrap();
    assert_eq!(source, deserialized);
}
