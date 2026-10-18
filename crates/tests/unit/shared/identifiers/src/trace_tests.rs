use std::collections::HashSet;
use systemprompt_identifiers::{DbValue, ToDbValue, TraceId};

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
    let ids: HashSet<String> = (0..20)
        .map(|_| TraceId::generate().as_str().to_string())
        .collect();
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

const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

#[test]
fn a_valid_traceparent_yields_its_trace_id() {
    let id = TraceId::from_traceparent(TRACEPARENT).expect("valid traceparent");
    assert_eq!(id.as_str(), "4bf92f3577b34da6a3ce929d0e0e4736");
}

#[test]
fn malformed_traceparents_are_rejected() {
    for header in [
        "",
        "garbage",
        "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
        "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
        "00-4bf92f3577b34da6a3ce929d0e0e473-00f067aa0ba902b7-01",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra",
    ] {
        assert!(TraceId::from_traceparent(header).is_none(), "{header}");
    }
}

#[test]
fn a_future_version_with_extra_fields_is_accepted() {
    let header = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-future";
    assert!(TraceId::from_traceparent(header).is_some());
}

#[test]
fn a_uuid_trace_id_has_a_w3c_form() {
    let id = TraceId::new("123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(
        id.w3c_hex().as_deref(),
        Some("123e4567e89b12d3a456426614174000")
    );
}

#[test]
fn a_non_w3c_trace_id_is_not_propagated() {
    assert!(TraceId::system().w3c_hex().is_none());
    assert!(TraceId::system().traceparent().is_none());
}

#[test]
fn an_outbound_traceparent_keeps_the_trace_id_and_round_trips() {
    let id = TraceId::from_traceparent(TRACEPARENT).expect("valid");
    let outbound = id.traceparent().expect("w3c id");
    assert!(outbound.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"));
    assert!(outbound.ends_with("-01"));
    assert_eq!(
        TraceId::from_traceparent(&outbound).expect("round trip"),
        id
    );
}
