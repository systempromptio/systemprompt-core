//! Metadata-only decoding of Desktop's OTLP/HTTP JSON log exports.
//!
//! Body text, scope names and severity text are deliberately absent from the
//! input types; only bounded attributes enter the shared telemetry sanitizer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::resource::v1::Resource;
use serde::Deserialize;
use systemprompt_identifiers::TraceId;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    #[serde(default)]
    resource_logs: Vec<InputResourceLogs>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputResourceLogs {
    #[serde(default)]
    resource: InputResource,
    #[serde(default)]
    scope_logs: Vec<InputScopeLogs>,
}

#[derive(Debug, Default, Deserialize)]
struct InputResource {
    #[serde(default)]
    attributes: Vec<InputAttribute>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputScopeLogs {
    #[serde(default)]
    log_records: Vec<InputLog>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputLog {
    #[serde(default)]
    time_unix_nano: InputInteger,
    #[serde(default)]
    observed_time_unix_nano: InputInteger,
    #[serde(default)]
    severity_number: InputInteger,
    #[serde(default)]
    trace_id: Option<TraceId>,
    #[serde(default)]
    span_id: String,
    #[serde(default)]
    attributes: Vec<InputAttribute>,
}

#[derive(Debug, Deserialize)]
struct InputAttribute {
    key: String,
    value: InputValue,
}

#[derive(Debug, Default, Deserialize)]
struct InputValue {
    #[serde(rename = "stringValue")]
    text: Option<String>,
    #[serde(rename = "boolValue")]
    boolean: Option<bool>,
    #[serde(rename = "intValue")]
    integer: Option<InputInteger>,
    #[serde(rename = "doubleValue")]
    decimal: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum InputInteger {
    Number(i64),
    Unsigned(u64),
    Text(String),
}

impl Default for InputInteger {
    fn default() -> Self {
        Self::Unsigned(0)
    }
}

impl InputInteger {
    fn unsigned(&self) -> u64 {
        match self {
            Self::Number(value) => u64::try_from(*value).unwrap_or(0),
            Self::Unsigned(value) => *value,
            Self::Text(value) => value.parse().unwrap_or(0),
        }
    }

    fn signed(&self) -> i64 {
        match self {
            Self::Number(value) => *value,
            Self::Unsigned(value) => i64::try_from(*value).unwrap_or(0),
            Self::Text(value) => value.parse().unwrap_or(0),
        }
    }
}

pub fn decode_logs(bytes: &[u8]) -> Result<ExportLogsServiceRequest, serde_json::Error> {
    let input: Export = serde_json::from_slice(bytes)?;
    let resource_logs = input
        .resource_logs
        .into_iter()
        .map(|resource| ResourceLogs {
            resource: Some(Resource {
                attributes: attributes(resource.resource.attributes),
                ..Default::default()
            }),
            scope_logs: resource
                .scope_logs
                .into_iter()
                .map(|scope| ScopeLogs {
                    log_records: scope
                        .log_records
                        .into_iter()
                        .map(|record| LogRecord {
                            time_unix_nano: record.time_unix_nano.unsigned(),
                            observed_time_unix_nano: record.observed_time_unix_nano.unsigned(),
                            severity_number: i32::try_from(record.severity_number.signed())
                                .unwrap_or(0),
                            trace_id: record
                                .trace_id
                                .as_ref()
                                .map_or_else(Vec::new, |id| hex(id.as_str(), 16)),
                            span_id: hex(&record.span_id, 8),
                            attributes: attributes(record.attributes),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
        .collect();
    Ok(ExportLogsServiceRequest { resource_logs })
}

fn attributes(input: Vec<InputAttribute>) -> Vec<KeyValue> {
    input
        .into_iter()
        .filter_map(|attribute| {
            if !super::convert::metadata_key(&attribute.key) {
                return None;
            }
            let value = attribute.value;
            let scalar = if let Some(text) = value.text {
                any_value::Value::StringValue(text)
            } else if let Some(boolean) = value.boolean {
                any_value::Value::BoolValue(boolean)
            } else if let Some(number) = value.integer {
                any_value::Value::IntValue(number.signed())
            } else {
                any_value::Value::DoubleValue(value.decimal?)
            };
            Some(KeyValue {
                key: attribute.key,
                key_strindex: 0,
                value: Some(AnyValue {
                    value: Some(scalar),
                }),
            })
        })
        .collect()
}

fn hex(value: &str, length: usize) -> Vec<u8> {
    if value.len() != length * 2 || !value.is_ascii() {
        return Vec::new();
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default()
}
