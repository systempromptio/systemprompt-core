//! Unit tests for OTLP value conversion: hex encoding, severity mapping, and
//! `AnyValue` to JSON/string flattening.

use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
use opentelemetry_proto::tonic::common::v1::{AnyValue, ArrayValue, KeyValue, KeyValueList};
use serde_json::json;
use systemprompt_api::routes::gateway::otel::test_api::{
    any_value_to_string, attrs_to_json, hex_lower, severity_to_level,
};
use systemprompt_logging::LogLevel;

fn any(v: AV) -> AnyValue {
    AnyValue { value: Some(v) }
}

fn kv(key: &str, v: AV) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        key_strindex: 0,
        value: Some(any(v)),
    }
}

#[test]
fn hex_lower_encodes_bytes() {
    assert_eq!(hex_lower(&[]), "");
    assert_eq!(hex_lower(&[0x00, 0x0f, 0xab, 0xff]), "000fabff");
}

#[test]
fn severity_maps_otlp_ranges_to_log_levels() {
    assert_eq!(severity_to_level(-3), LogLevel::Trace);
    assert_eq!(severity_to_level(4), LogLevel::Trace);
    assert_eq!(severity_to_level(5), LogLevel::Debug);
    assert_eq!(severity_to_level(8), LogLevel::Debug);
    assert_eq!(severity_to_level(9), LogLevel::Info);
    assert_eq!(severity_to_level(12), LogLevel::Info);
    assert_eq!(severity_to_level(13), LogLevel::Warn);
    assert_eq!(severity_to_level(16), LogLevel::Warn);
    assert_eq!(severity_to_level(17), LogLevel::Error);
    assert_eq!(severity_to_level(24), LogLevel::Error);
}

#[test]
fn any_value_to_string_flattens_scalars() {
    assert_eq!(any_value_to_string(None), "");
    assert_eq!(any_value_to_string(Some(&AnyValue { value: None })), "");
    assert_eq!(
        any_value_to_string(Some(&any(AV::StringValue("hi".to_owned())))),
        "hi"
    );
    assert_eq!(any_value_to_string(Some(&any(AV::BoolValue(true)))), "true");
    assert_eq!(any_value_to_string(Some(&any(AV::IntValue(-7)))), "-7");
    assert_eq!(any_value_to_string(Some(&any(AV::DoubleValue(1.5)))), "1.5");
    assert_eq!(
        any_value_to_string(Some(&any(AV::BytesValue(vec![1, 2, 3])))),
        "<bytes:3>"
    );
}

#[test]
fn any_value_to_string_serialises_composites_as_json() {
    let arr = any(AV::ArrayValue(ArrayValue {
        values: vec![any(AV::IntValue(1)), any(AV::StringValue("x".to_owned()))],
    }));
    assert_eq!(any_value_to_string(Some(&arr)), "[1,\"x\"]");

    let kvs = any(AV::KvlistValue(KeyValueList {
        values: vec![kv("k", AV::BoolValue(false))],
    }));
    assert_eq!(any_value_to_string(Some(&kvs)), "{\"k\":false}");
}

#[test]
fn attrs_to_json_builds_nested_object() {
    let attrs = vec![
        kv("service.name", AV::StringValue("api".to_owned())),
        kv("retries", AV::IntValue(3)),
        kv(
            "nested",
            AV::KvlistValue(KeyValueList {
                values: vec![kv("inner", AV::DoubleValue(0.25))],
            }),
        ),
        kv("payload", AV::BytesValue(vec![0; 4])),
        KeyValue {
            key: "missing".to_owned(),
            key_strindex: 0,
            value: None,
        },
    ];
    let value = attrs_to_json(&attrs);
    assert_eq!(
        value,
        json!({
            "service.name": "api",
            "retries": 3,
            "nested": {"inner": 0.25},
            "payload": "<bytes:4>",
            "missing": null,
        })
    );
}

#[test]
fn attrs_to_json_empty_is_empty_object() {
    assert_eq!(attrs_to_json(&[]), json!({}));
}
