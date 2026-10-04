//! OTLP protobuf value conversion into JSON and log levels.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_logging::LogLevel;

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub const fn severity_to_level(severity_number: i32) -> LogLevel {
    match severity_number {
        ..=4 => LogLevel::Trace,
        5..=8 => LogLevel::Debug,
        9..=12 => LogLevel::Info,
        13..=16 => LogLevel::Warn,
        _ => LogLevel::Error,
    }
}

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

// JSON: OTLP `AnyValue` attributes — rendered to JSON for span storage.
pub fn attrs_to_json(attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue]) -> Value {
    let mut map = serde_json::Map::new();
    for kv in attrs {
        if !metadata_key(&kv.key) {
            continue;
        }
        let value = any_value_to_json(kv.value.as_ref());
        if value.is_number()
            || value.is_boolean()
            || value.as_str().is_some_and(|s| {
                s.len() <= 128
                    && s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-._:/[] ".contains(c))
                    && !s.starts_with("sk-")
            })
        {
            map.insert(kv.key.clone(), value);
        }
    }
    Value::Object(map)
}

// JSON: OTLP `AnyValue` attributes — rendered to JSON for span storage.
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


pub fn metadata_key(key: &str) -> bool {
    matches!(
        key,
        "service.name"
            | "service.version"
            | "deployment.environment"
            | "os.type"
            | "gen_ai.system"
            | "gen_ai.request.model"
            | "gen_ai.response.model"
            | "gen_ai.usage.input_tokens"
            | "gen_ai.usage.output_tokens"
            | "gen_ai.usage.cache_read_input_tokens"
            | "gen_ai.usage.cache_creation_input_tokens"
            | "http.response.status_code"
            | "http.status_code"
            | "error.type"
            | "systemprompt.user.id"
            | "systemprompt.session.id"
            | "systemprompt.request.id"
            | "systemprompt.context.capacity"
            | "systemprompt.request.bytes"
            | "systemprompt.compaction.count"
    )
}
