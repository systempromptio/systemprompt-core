//! Live OTLP export: the id and retry primitives, the log-record mapping and
//! delivery of a fed entry to a collector when the exporter drains.

use std::time::Duration;

use chrono::{TimeZone, Utc};
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::logs::v1::SeverityNumber;
use prost::Message;
use systemprompt_identifiers::{InstanceId, SessionId, TraceId, UserId};
use systemprompt_logging::otlp::{
    OtlpFeed, RETRY_DELAYS, is_retryable, severity_number, span_id_bytes, to_log_record,
    trace_id_bytes, unix_nanos,
};
use systemprompt_logging::{LogActor, LogEntry, LogLevel};
use systemprompt_manifest::profile::{OtlpExportConfig, OtlpProtocol, OtlpSignal};
use systemprompt_traits::BackgroundTasks;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn entry(level: LogLevel, message: &str) -> LogEntry {
    LogEntry::new(
        level,
        "otlp_test",
        message,
        LogActor::new(
            UserId::new("user-otlp"),
            SessionId::new("session-otlp"),
            TraceId::new("trace_otlp"),
        ),
    )
}

fn config(endpoint: String) -> OtlpExportConfig {
    OtlpExportConfig {
        endpoint,
        protocol: OtlpProtocol::Http,
        headers: Default::default(),
        signals: vec![OtlpSignal::Logs],
        batch_seconds: 3600,
    }
}

#[test]
fn trace_ids_pass_through_w3c_hex_and_digest_everything_else() {
    let w3c = "4bf92f3577b34da6a3ce929d0e0e4736";
    assert_eq!(hex::encode(trace_id_bytes(w3c)), w3c);
    let derived = trace_id_bytes("trace_abc");
    assert_eq!(derived.len(), 16);
    assert_eq!(derived, trace_id_bytes("trace_abc"));
    assert_ne!(derived, trace_id_bytes("trace_abd"));
    assert_ne!(hex::encode(trace_id_bytes(&"0".repeat(32))), "0".repeat(32));
}

#[test]
fn span_ids_are_eight_bytes_keyed_by_kind_and_key() {
    let a = span_id_bytes("span", "r1");
    assert_eq!(a.len(), 8);
    assert_eq!(a, span_id_bytes("span", "r1"));
    assert_ne!(a, span_id_bytes("other", "r1"));
    assert_ne!(a, span_id_bytes("span", "r2"));
}

#[test]
fn unix_nanos_is_epoch_based_and_never_negative() {
    let at = Utc.timestamp_opt(1_700_000_000, 0).single().expect("valid");
    assert_eq!(unix_nanos(at), 1_700_000_000 * 1_000_000_000);
    assert_eq!(
        unix_nanos(Utc.timestamp_opt(-5, 0).single().expect("valid")),
        0
    );
}

#[test]
fn retry_policy_backs_off_and_only_retries_transient_failures() {
    assert!(RETRY_DELAYS.windows(2).all(|w| w[0] < w[1]));
    assert!(is_retryable(None));
    assert!(is_retryable(Some(reqwest::StatusCode::TOO_MANY_REQUESTS)));
    assert!(is_retryable(Some(reqwest::StatusCode::SERVICE_UNAVAILABLE)));
    assert!(!is_retryable(Some(reqwest::StatusCode::UNAUTHORIZED)));
    assert!(!is_retryable(Some(reqwest::StatusCode::BAD_REQUEST)));
}

#[test]
fn log_records_carry_level_body_attribution_and_trace_correlation() {
    let record = to_log_record(&entry(LogLevel::Warn, "disk nearly full"));
    assert_eq!(record.severity_number, SeverityNumber::Warn as i32);
    assert_eq!(record.severity_text, "WARN");
    assert_eq!(
        record.body.and_then(|b| b.value),
        Some(Value::StringValue("disk nearly full".to_owned()))
    );
    assert_eq!(record.trace_id, trace_id_bytes("trace_otlp"));
    let user = record
        .attributes
        .iter()
        .find(|kv| kv.key == "enduser.id")
        .and_then(|kv| kv.value.as_ref())
        .and_then(|v| v.value.clone());
    assert_eq!(user, Some(Value::StringValue("user-otlp".to_owned())));
    assert_eq!(severity_number(LogLevel::Error), SeverityNumber::Error);
}

#[tokio::test]
async fn a_fed_entry_reaches_the_collector_when_the_exporter_drains() {
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&collector)
        .await;
    let tasks = BackgroundTasks::new();
    let feed = OtlpFeed::spawn(
        &config(collector.uri()),
        Some(InstanceId::new("logging-test")),
        &tasks,
    );

    feed.log(&entry(LogLevel::Info, "shipped live"));
    let outcome = tasks.shutdown(Duration::from_secs(10)).await;

    assert!(outcome.is_drained());
    let requests = collector.received_requests().await.expect("recorded");
    let decoded = ExportLogsServiceRequest::decode(requests[0].body.as_slice())
        .expect("collector receives a protobuf OTLP log envelope");
    let records = &decoded.resource_logs[0].scope_logs[0].log_records;
    assert_eq!(records.len(), 1);
    assert_eq!(feed.dropped(), 0);
}

#[tokio::test]
async fn a_refused_batch_is_dropped_and_counted() {
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&collector)
        .await;
    let tasks = BackgroundTasks::new();
    let feed = OtlpFeed::spawn(&config(collector.uri()), None, &tasks);

    feed.log(&entry(LogLevel::Info, "refused"));
    feed.log(&entry(LogLevel::Info, "refused too"));
    let outcome = tasks.shutdown(Duration::from_secs(10)).await;

    assert!(outcome.is_drained());
    assert_eq!(feed.dropped(), 2);
}
