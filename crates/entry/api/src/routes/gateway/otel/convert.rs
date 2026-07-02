//! OTLP protobuf value conversion into JSON and log levels.

use serde_json::{Value, json};
use systemprompt_logging::LogLevel;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub const fn severity_to_level(severity_number: i32) -> LogLevel {
    match severity_number {
        ..=4 => LogLevel::Trace,
        5..=8 => LogLevel::Debug,
        9..=12 => LogLevel::Info,
        13..=16 => LogLevel::Warn,
        _ => LogLevel::Error,
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn any_value_to_string(
    value: Option<&opentelemetry_proto::tonic::common::v1::AnyValue>,
) -> String {
    use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
    let Some(av) = value.and_then(|v| v.value.as_ref()) else {
        return String::new();
    };
    match av {
        AV::StringValue(s) => s.clone(),
        AV::BoolValue(b) => b.to_string(),
        AV::IntValue(i) => i.to_string(),
        AV::DoubleValue(f) => f.to_string(),
        AV::BytesValue(b) => format!("<bytes:{}>", b.len()),
        AV::StringValueStrindex(idx) => format!("<strindex:{idx}>"),
        AV::ArrayValue(_) | AV::KvlistValue(_) => serde_json::to_string(&any_value_to_json(value))
            .unwrap_or_else(|e| format!("<json-serialise-failed: {e}>")),
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn attrs_to_json(attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue]) -> Value {
    let mut map = serde_json::Map::new();
    for kv in attrs {
        map.insert(kv.key.clone(), any_value_to_json(kv.value.as_ref()));
    }
    Value::Object(map)
}

fn any_value_to_json(value: Option<&opentelemetry_proto::tonic::common::v1::AnyValue>) -> Value {
    use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
    let Some(av) = value.and_then(|v| v.value.as_ref()) else {
        return Value::Null;
    };
    match av {
        AV::StringValue(s) => Value::String(s.clone()),
        AV::BoolValue(b) => Value::Bool(*b),
        AV::IntValue(i) => Value::from(*i),
        AV::DoubleValue(f) => json!(f),
        AV::BytesValue(b) => Value::String(format!("<bytes:{}>", b.len())),
        AV::StringValueStrindex(idx) => Value::String(format!("<strindex:{idx}>")),
        AV::ArrayValue(arr) => Value::Array(
            arr.values
                .iter()
                .map(|v| any_value_to_json(Some(v)))
                .collect(),
        ),
        AV::KvlistValue(kvs) => {
            let mut map = serde_json::Map::new();
            for kv in &kvs.values {
                map.insert(kv.key.clone(), any_value_to_json(kv.value.as_ref()));
            }
            Value::Object(map)
        },
    }
}
