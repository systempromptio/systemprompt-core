use opentelemetry_proto::tonic::common::v1::KeyValue;
use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
use systemprompt_api::routes::gateway::otel::json::decode_logs;

fn value_of<'a>(attrs: &'a [KeyValue], key: &str) -> Option<&'a AV> {
    attrs
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| kv.value.as_ref())
        .and_then(|v| v.value.as_ref())
}

const FULL: &str = r#"{
  "resourceLogs": [{
    "resource": {"attributes": [
      {"key": "service.name", "value": {"stringValue": "claude-desktop"}},
      {"key": "host.name", "value": {"stringValue": "secret-laptop"}}
    ]},
    "scopeLogs": [{
      "logRecords": [{
        "timeUnixNano": "1700000000000000000",
        "observedTimeUnixNano": 1700000000000000001,
        "severityNumber": 9,
        "traceId": "0123456789abcdef0123456789abcdef",
        "spanId": "0123456789abcdef",
        "body": {"stringValue": "prompt text must not survive"},
        "attributes": [
          {"key": "gen_ai.usage.input_tokens", "value": {"intValue": "42"}},
          {"key": "gen_ai.usage.output_tokens", "value": {"intValue": 7}},
          {"key": "http.status_code", "value": {"doubleValue": 200.5}},
          {"key": "error.type", "value": {"boolValue": true}},
          {"key": "gen_ai.system", "value": {}},
          {"key": "user.prompt", "value": {"stringValue": "drop me"}}
        ]
      }]
    }]
  }]
}"#;

#[test]
fn decodes_metadata_and_drops_everything_else() {
    let request = decode_logs(FULL.as_bytes()).expect("valid export");
    assert_eq!(request.resource_logs.len(), 1);
    let resource = &request.resource_logs[0];
    let resource_attrs = &resource.resource.as_ref().expect("resource").attributes;
    assert_eq!(resource_attrs.len(), 1);
    assert!(matches!(
        value_of(resource_attrs, "service.name"),
        Some(AV::StringValue(s)) if s == "claude-desktop"
    ));

    let record = &resource.scope_logs[0].log_records[0];
    assert_eq!(record.time_unix_nano, 1_700_000_000_000_000_000);
    assert_eq!(record.observed_time_unix_nano, 1_700_000_000_000_000_001);
    assert_eq!(record.severity_number, 9);
    assert_eq!(record.trace_id.len(), 16);
    assert_eq!(record.trace_id[0], 0x01);
    assert_eq!(
        record.span_id,
        vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
    );
    assert!(record.body.is_none());

    let attrs = &record.attributes;
    assert!(matches!(
        value_of(attrs, "gen_ai.usage.input_tokens"),
        Some(AV::IntValue(42))
    ));
    assert!(matches!(
        value_of(attrs, "gen_ai.usage.output_tokens"),
        Some(AV::IntValue(7))
    ));
    assert!(
        matches!(value_of(attrs, "http.status_code"), Some(AV::DoubleValue(d)) if (*d - 200.5).abs() < f64::EPSILON)
    );
    assert!(matches!(
        value_of(attrs, "error.type"),
        Some(AV::BoolValue(true))
    ));
    assert!(
        value_of(attrs, "gen_ai.system").is_none(),
        "an empty value is dropped"
    );
    assert!(
        value_of(attrs, "user.prompt").is_none(),
        "non-metadata keys are dropped"
    );
    assert_eq!(attrs.len(), 4);
}

#[test]
fn malformed_ids_and_integers_decode_to_empty_defaults() {
    let body = r#"{"resourceLogs": [{"scopeLogs": [{"logRecords": [{
        "timeUnixNano": "not-a-number",
        "observedTimeUnixNano": -5,
        "severityNumber": 99999999999,
        "traceId": "short",
        "spanId": "zzzzzzzzzzzzzzzz"
    }]}]}]}"#;
    let request = decode_logs(body.as_bytes()).expect("valid json");
    let record = &request.resource_logs[0].scope_logs[0].log_records[0];
    assert_eq!(record.time_unix_nano, 0);
    assert_eq!(record.observed_time_unix_nano, 0);
    assert_eq!(record.severity_number, 0);
    assert!(record.trace_id.is_empty());
    assert!(record.span_id.is_empty());
}

#[test]
fn an_empty_object_decodes_to_no_logs() {
    let request = decode_logs(b"{}").expect("empty export");
    assert!(request.resource_logs.is_empty());
}

#[test]
fn invalid_json_is_rejected() {
    assert!(decode_logs(b"not json").is_err());
}
