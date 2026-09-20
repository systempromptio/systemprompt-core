//! OTLP export coverage: cursor arithmetic and conversion alongside durable
//! database-to-collector delivery behavior.

use chrono::{DateTime, Duration, TimeZone, Utc};
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::trace::v1::Span;
use prost::Message;
use std::sync::{Arc, Mutex};
use systemprompt_identifiers::{ContextId, SessionId, TraceId, UserId};
use systemprompt_models::profile::{OtlpExportConfig, OtlpProtocol, OtlpSignal};
use systemprompt_scheduler::jobs::otlp_export::{
    GOVERNANCE_SPAN, GovernanceRow, LedgerRow, LogRow, OtlpExportJob, REQUEST_SPAN, RETRY_DELAYS,
    RequestRow, TOOL_SPAN, TraceBatch, Watermark, is_retryable, pacing_elapsed, severity_number,
    span_id_bytes, to_log_record, to_spans, trace_id_bytes, unix_nanos,
};
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_traits::Job;
use tracing_subscriber::layer::SubscriberExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Clone, Default)]
struct DiagnosticWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for DiagnosticWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("diagnostic buffer")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for DiagnosticWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl DiagnosticWriter {
    fn events(&self) -> Vec<serde_json::Value> {
        String::from_utf8(self.0.lock().expect("diagnostic buffer").clone())
            .expect("JSON diagnostics are UTF-8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON diagnostic"))
            .collect()
    }
}

// Why: OTLP `Status.code` — 0 UNSET, 1 OK, 2 ERROR.
const STATUS_OK: i32 = 1;
const STATUS_ERROR: i32 = 2;

fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000 + secs, 0)
        .single()
        .expect("valid")
}

fn request(id: &str, trace_id: Option<&str>, status: &str) -> RequestRow {
    RequestRow {
        id: id.to_owned(),
        request_id: format!("req-{id}"),
        user_id: UserId::new("user-1"),
        session_id: Some(SessionId::new("sess-1")),
        context_id: ContextId::try_new("3f2a1c4e-8b7d-4c2a-9e1f-0a1b2c3d4e5f").expect("v4"),
        trace_id: trace_id.map(TraceId::new),
        provider: Some("anthropic".to_owned()),
        served_provider: Some("vertex".to_owned()),
        model: Some("claude-sonnet-5".to_owned()),
        requested_model: Some("claude-sonnet-5".to_owned()),
        route_match: Some("failover:anthropic->vertex".to_owned()),
        input_tokens: Some(120),
        output_tokens: Some(40),
        cache_read_tokens: None,
        cache_creation_tokens: None,
        cost_microdollars: 2_500,
        latency_ms: Some(900),
        upstream_latency_ms: Some(850),
        finish_reason: Some("end_turn".to_owned()),
        status: status.to_owned(),
        error_message: (status == "failed").then(|| "upstream 500".to_owned()),
        client_kind: "claude-code".to_owned(),
        wire_protocol: "anthropic.messages".to_owned(),
        request_kind: "turn".to_owned(),
        actor_kind: "user".to_owned(),
        actor_id: "user-1".to_owned(),
        instance_id: Some("node-a".to_owned()),
        created_at: at(0),
        completed_at: at(1),
    }
}

fn ledger(request_id: &str, call_id: &str, failed: bool) -> LedgerRow {
    LedgerRow {
        ai_tool_call_id: Some(call_id.to_owned()),
        request_id: Some(request_id.to_owned()),
        mcp_execution_id: Some(format!("exec-{call_id}")),
        tool_name: Some("read_file".to_owned()),
        server_name: Some("fs".to_owned()),
        intended_at: Some(at(0)),
        executed_at: Some(at(0)),
        completed_at: None,
        execution_time_ms: Some(250),
        execution_status: Some(if failed { "failed" } else { "success" }.to_owned()),
        error_message: failed.then(|| "boom".to_owned()),
        source: Some("hook_claude_code".to_owned()),
        state: Some("executed".to_owned()),
        is_error: Some(failed),
        artifact_type: Some("tool_result".to_owned()),
        payload_bytes: Some(1024),
        secret_redactions: Some(0),
        occurred_at: Some(at(0)),
    }
}

fn decision(id: &str, trace_id: &str, verdict: &str) -> GovernanceRow {
    GovernanceRow {
        id: id.to_owned(),
        trace_id: Some(TraceId::new(trace_id)),
        tool_name: "read_file".to_owned(),
        decision: verdict.to_owned(),
        policy: "scope".to_owned(),
        reason: "out of scope".to_owned(),
        plugin_id: Some("astound-commons".to_owned()),
        actor_kind: "user".to_owned(),
        actor_id: "user-1".to_owned(),
        tool_use_id: Some("call-1".to_owned()),
        created_at: at(0),
    }
}

fn attr<'a>(span: &'a Span, key: &str) -> Option<&'a str> {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::StringValue(s) => Some(s.as_str()),
            _ => None,
        })
}

#[test]
fn job_identity_and_schedule() {
    assert_eq!(OtlpExportJob.name(), "otlp_export");
    assert!(!OtlpExportJob.description().is_empty());
    assert!(
        tokio_cron_scheduler::Job::new_async(OtlpExportJob.schedule(), |_uuid, _lock| Box::pin(
            async {}
        ))
        .is_ok()
    );
}

#[test]
fn watermark_admits_strictly_later_rows() {
    let cursor = Watermark::new(at(10), "b");
    assert!(cursor.admits(at(11), "a"));
    assert!(cursor.admits(at(10), "c"));
    assert!(!cursor.admits(at(10), "b"));
    assert!(!cursor.admits(at(10), "a"));
    assert!(!cursor.admits(at(9), "z"));
}

#[test]
fn pacing_honours_batch_seconds() {
    let now = at(100);
    assert!(pacing_elapsed(None, now, 15));
    assert!(pacing_elapsed(Some(now - Duration::seconds(15)), now, 15));
    assert!(!pacing_elapsed(Some(now - Duration::seconds(14)), now, 15));
    assert!(pacing_elapsed(Some(now - Duration::seconds(1)), now, 1));
}

#[test]
fn trace_ids_pass_through_w3c_hex_and_digest_everything_else() {
    let w3c = "4bf92f3577b34da6a3ce929d0e0e4736";
    assert_eq!(hex::encode(trace_id_bytes(w3c)), w3c);
    let derived = trace_id_bytes("trace_abc");
    assert_eq!(derived.len(), 16);
    assert_eq!(derived, trace_id_bytes("trace_abc"));
    assert_ne!(derived, trace_id_bytes("trace_abd"));
    // Why: 32 zero hex chars is the OTLP "absent" trace id; it must be digested.
    assert_ne!(hex::encode(trace_id_bytes(&"0".repeat(32))), "0".repeat(32));
}

#[test]
fn span_ids_are_eight_bytes_keyed_by_kind_and_key() {
    let a = span_id_bytes(REQUEST_SPAN, "r1");
    assert_eq!(a.len(), 8);
    assert_eq!(a, span_id_bytes(REQUEST_SPAN, "r1"));
    assert_ne!(a, span_id_bytes(TOOL_SPAN, "r1"));
    assert_ne!(a, span_id_bytes(REQUEST_SPAN, "r2"));
}

#[test]
fn unix_nanos_is_epoch_based_and_never_negative() {
    assert_eq!(unix_nanos(at(0)), 1_700_000_000 * 1_000_000_000);
    assert_eq!(
        unix_nanos(Utc.timestamp_opt(-5, 0).single().expect("valid")),
        0
    );
}

fn export_config(endpoint: String) -> OtlpExportConfig {
    OtlpExportConfig {
        endpoint,
        protocol: OtlpProtocol::Http,
        headers: Default::default(),
        signals: vec![OtlpSignal::Logs],
        batch_seconds: 1,
    }
}

async fn seed_log(pool: &sqlx::PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message) \
         VALUES ($1, NOW() - INTERVAL '10 seconds', 'INFO', 'otlp-test', 'durable export')",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO otlp_export_state (signal, watermark, watermark_id) \
         VALUES ('logs', to_timestamp(0), '') \
         ON CONFLICT (signal) DO UPDATE SET watermark = EXCLUDED.watermark, watermark_id = ''",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_trace_request(pool: &sqlx::PgPool, id: &str, trace: &str) {
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, trace_id, provider, model, \
         actor_kind, actor_id, client_kind, wire_protocol, status, completed_at) \
         VALUES ($1, $2, 'otlp-user', '3f2a1c4e-8b7d-4c2a-9e1f-0a1b2c3d4e5f', $3, 'anthropic', \
         'claude-test', 'user', 'otlp-user', 'claude-code', 'anthropic.messages', 'completed', \
         NOW() - INTERVAL '10 seconds')",
    )
    .bind(id)
    .bind(format!("request-{id}"))
    .bind(trace)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, decision, policy, reason, \
         actor_kind, actor_id, context_id, trace_id, created_at) VALUES ($1, 'otlp-user', \
         'otlp-session', 'read_file', 'deny', 'scope', 'outside scope', 'user', 'otlp-user', \
         '3f2a1c4e-8b7d-4c2a-9e1f-0a1b2c3d4e5f', $2, NOW() - INTERVAL '10 seconds')",
    )
    .bind(format!("decision-{id}"))
    .bind(trace)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO otlp_export_state (signal, watermark, watermark_id) VALUES ('traces', to_timestamp(0), '') \
         ON CONFLICT (signal) DO UPDATE SET watermark = EXCLUDED.watermark, watermark_id = ''",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn traces_export_keeps_request_and_governance_decision_correlated_in_one_protobuf_batch() {
    let db = DisposableDb::installed("otlp_export_traces").await.unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().unwrap();
    seed_trace_request(raw.as_ref(), "otlp-trace-row", "trace-otlp-correlation").await;
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&collector)
        .await;
    let mut config = export_config(collector.uri());
    config.signals = vec![OtlpSignal::Traces];
    let report = systemprompt_scheduler::otlp_export_now(&raw, &config, None)
        .await
        .unwrap();
    assert_eq!(report.signals[0].rows, 1);
    let request = &collector.received_requests().await.unwrap()[0];
    let decoded =
        opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest::decode(
            request.body.as_slice(),
        )
        .unwrap();
    let spans = &decoded.resource_spans[0].scope_spans[0].spans;
    assert_eq!(spans.len(), 2, "request plus its governance decision");
    assert_eq!(spans[0].trace_id, spans[1].trace_id);
    assert_eq!(spans[1].parent_span_id, spans[0].span_id);
    let state = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone())
        .get_or_start(OtlpSignal::Traces)
        .await
        .unwrap();
    assert_eq!(state.watermark_id, "otlp-trace-row");
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}

#[tokio::test]
async fn export_now_posts_a_decodable_log_batch_and_advances_the_durable_watermark() {
    let db = DisposableDb::installed("otlp_export_ack").await.unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().unwrap();
    seed_log(raw.as_ref(), "otlp-log-ack").await;
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&collector)
        .await;

    let report = systemprompt_scheduler::otlp_export_now(
        &raw,
        &export_config(collector.uri()),
        Some("scheduler-test"),
    )
    .await
    .unwrap();
    assert_eq!(report.signals.len(), 1);
    assert_eq!(report.signals[0].rows, 1);
    assert!(report.signals[0].error.is_none());

    let requests = collector.received_requests().await.unwrap();
    let request = &requests[0];
    assert_eq!(
        request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/x-protobuf")
    );
    let decoded =
        opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest::decode(
            request.body.as_slice(),
        )
        .expect("collector receives a protobuf OTLP log envelope");
    assert_eq!(decoded.resource_logs.len(), 1);
    let state = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone())
        .get_or_start(OtlpSignal::Logs)
        .await
        .unwrap();
    assert_eq!(state.watermark_id, "otlp-log-ack");
    assert_eq!(state.rows_total, 1);
    assert_eq!(state.failures_total, 0);
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}

#[tokio::test]
async fn rejected_collector_keeps_the_cursor_then_a_retry_ships_the_identical_batch() {
    let db = DisposableDb::installed("otlp_export_retry").await.unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().unwrap();
    seed_log(raw.as_ref(), "otlp-log-retry").await;
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(400).set_body_string("schema rejected"))
        .up_to_n_times(1)
        .mount(&collector)
        .await;

    let config = export_config(collector.uri());
    let failed = systemprompt_scheduler::otlp_export_now(&raw, &config, None)
        .await
        .unwrap();
    assert!(failed.signals[0].error.as_deref().unwrap().contains("400"));
    let repository = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone());
    let after_failure = repository.get_or_start(OtlpSignal::Logs).await.unwrap();
    assert_eq!(after_failure.watermark_id, "");
    assert_eq!(after_failure.failures_total, 1);

    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&collector)
        .await;
    let retried = systemprompt_scheduler::otlp_export_now(&raw, &config, None)
        .await
        .unwrap();
    assert_eq!(retried.signals[0].rows, 1);
    let requests = collector.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].body, requests[1].body,
        "cursor failure retries the same batch"
    );
    let after_success = repository.get_or_start(OtlpSignal::Logs).await.unwrap();
    assert_eq!(after_success.watermark_id, "otlp-log-retry");
    assert_eq!(after_success.rows_total, 1);
    assert_eq!(after_success.failures_total, 1);
    assert!(after_success.last_error.is_none());
    drop(repository);
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}

#[tokio::test]
async fn invalid_header_keeps_the_cursor_then_repaired_config_delivers_the_batch() {
    let diagnostics = DiagnosticWriter::default();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(false)
            .with_writer(diagnostics.clone()),
    );
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let db = DisposableDb::installed("otlp_invalid_header")
        .await
        .unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().unwrap();
    seed_log(raw.as_ref(), "otlp-log-invalid-header").await;
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&collector)
        .await;
    let mut config = export_config(collector.uri());
    config.headers.insert(
        "invalid header name".to_owned(),
        "credential-sentinel".to_owned(),
    );

    let failed = systemprompt_scheduler::otlp_export_now(&raw, &config, None)
        .await
        .expect("signal failures are reported, not returned");
    assert_eq!(failed.failed(), 1);
    assert_eq!(failed.rows(), 0);
    assert!(
        failed.signals[0]
            .error
            .as_deref()
            .is_some_and(|message| message.contains("invalid header invalid header name"))
    );
    assert!(collector.received_requests().await.unwrap().is_empty());
    let repository = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone());
    let after_failure = repository.get_or_start(OtlpSignal::Logs).await.unwrap();
    assert_eq!(after_failure.watermark_id, "");
    assert_eq!(after_failure.failures_total, 1);
    let failed_event = diagnostics
        .events()
        .into_iter()
        .find(|event| event["fields"]["message"] == "OTLP export batch failed")
        .expect("failure diagnostic");
    assert_eq!(failed_event["fields"]["signal"], "logs");
    assert!(
        failed_event["fields"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("invalid header invalid header name"))
    );
    assert!(!failed_event.to_string().contains("credential-sentinel"));

    config.headers.clear();
    let repaired = systemprompt_scheduler::otlp_export_now(&raw, &config, None)
        .await
        .expect("repaired collector config");
    assert_eq!(repaired.failed(), 0);
    assert_eq!(repaired.rows(), 1);
    let requests = collector.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let after_repair = repository.get_or_start(OtlpSignal::Logs).await.unwrap();
    assert_eq!(after_repair.watermark_id, "otlp-log-invalid-header");
    assert_eq!(after_repair.rows_total, 1);
    assert_eq!(after_repair.failures_total, 1);
    assert!(after_repair.last_error.is_none());
    let exported_event = diagnostics
        .events()
        .into_iter()
        .find(|event| event["fields"]["message"] == "OTLP batch exported")
        .expect("recovery diagnostic");
    assert_eq!(exported_event["fields"]["signal"], "logs");
    assert_eq!(exported_event["fields"]["rows"], 1);

    drop(repository);
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}

#[tokio::test]
async fn a_retryable_503_retries_the_same_protobuf_payload_before_advancing() {
    let db = DisposableDb::installed("otlp_export_503").await.unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().unwrap();
    seed_log(raw.as_ref(), "otlp-log-503").await;
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(503).set_body_string("collector busy"))
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&collector)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .respond_with(ResponseTemplate::new(200))
        .with_priority(10)
        .expect(1)
        .mount(&collector)
        .await;

    let report =
        systemprompt_scheduler::otlp_export_now(&raw, &export_config(collector.uri()), None)
            .await
            .unwrap();
    assert_eq!(report.signals[0].rows, 1);
    let requests = collector.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "one retry follows the 503");
    assert_eq!(requests[0].body, requests[1].body);
    let state = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone())
        .get_or_start(OtlpSignal::Logs)
        .await
        .unwrap();
    assert_eq!(state.watermark_id, "otlp-log-503");
    assert_eq!(
        state.failures_total, 0,
        "in-run recovery is not a durable failure"
    );
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}

#[tokio::test]
async fn an_empty_signal_marks_the_cursor_caught_up_without_posting_to_the_collector() {
    let db = DisposableDb::installed("otlp_export_empty").await.unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().unwrap();
    let collector = MockServer::start().await;

    let report =
        systemprompt_scheduler::otlp_export_now(&raw, &export_config(collector.uri()), None)
            .await
            .unwrap();
    assert_eq!(report.signals[0].rows, 0);
    assert!(collector.received_requests().await.unwrap().is_empty());
    let state = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone())
        .get_or_start(OtlpSignal::Logs)
        .await
        .unwrap();
    assert!(state.last_success_at.is_some());
    assert_eq!(state.batches_total, 0);
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}

#[test]
fn one_request_span_with_tool_and_governance_children() {
    let batch = TraceBatch {
        requests: vec![request("r1", Some("trace-1"), "completed")],
        ledger: vec![ledger("r1", "call-1", false), ledger("r1", "call-2", true)],
        governance: vec![
            decision("g1", "trace-1", "allow"),
            decision("g2", "trace-1", "deny"),
        ],
    };
    let spans = to_spans(&batch);
    assert_eq!(spans.len(), 5);

    let root = &spans[0];
    assert_eq!(root.name, "ai_request claude-sonnet-5");
    assert!(root.parent_span_id.is_empty());
    assert_eq!(root.trace_id, trace_id_bytes("trace-1"));
    assert_eq!(root.span_id, span_id_bytes(REQUEST_SPAN, "r1"));
    assert_eq!(root.status.as_ref().map(|s| s.code), Some(STATUS_OK));
    assert_eq!(attr(root, "gen_ai.system"), Some("anthropic"));
    assert_eq!(attr(root, "systemprompt.served_provider"), Some("vertex"));
    assert_eq!(attr(root, "enduser.id"), Some("user-1"));
    assert_eq!(attr(root, "session.id"), Some("sess-1"));
    assert_eq!(attr(root, "systemprompt.trace.id"), Some("trace-1"));
    assert!(
        root.attributes
            .iter()
            .any(|kv| kv.key == "gen_ai.usage.input_tokens")
    );
    assert!(
        root.attributes
            .iter()
            .any(|kv| kv.key == "systemprompt.cost.usd")
    );
    assert_eq!(
        root.end_time_unix_nano - root.start_time_unix_nano,
        1_000_000_000
    );

    for child in &spans[1..] {
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id, root.span_id);
    }
    let tools: Vec<_> = spans
        .iter()
        .filter(|s| s.name.starts_with(TOOL_SPAN))
        .collect();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].status.as_ref().map(|s| s.code), Some(STATUS_OK));
    assert_eq!(tools[1].status.as_ref().map(|s| s.code), Some(STATUS_ERROR));
    assert_eq!(
        tools[0].end_time_unix_nano - tools[0].start_time_unix_nano,
        250_000_000
    );
    let decisions: Vec<_> = spans
        .iter()
        .filter(|s| s.name.starts_with(GOVERNANCE_SPAN))
        .collect();
    assert_eq!(decisions.len(), 2);
    assert_eq!(
        decisions[1].status.as_ref().map(|s| s.code),
        Some(STATUS_ERROR)
    );
    assert_eq!(
        attr(decisions[1], "systemprompt.governance.decision"),
        Some("deny")
    );
}

#[test]
fn children_attach_only_to_their_own_request() {
    let batch = TraceBatch {
        requests: vec![
            request("r1", Some("trace-1"), "completed"),
            request("r2", None, "failed"),
        ],
        ledger: vec![ledger("r2", "call-9", false)],
        governance: vec![decision("g1", "trace-1", "allow")],
    };
    let spans = to_spans(&batch);
    assert_eq!(spans.len(), 4);
    let r1 = span_id_bytes(REQUEST_SPAN, "r1");
    let r2 = span_id_bytes(REQUEST_SPAN, "r2");
    let gov = spans
        .iter()
        .find(|s| s.name.starts_with(GOVERNANCE_SPAN))
        .expect("decision");
    assert_eq!(gov.parent_span_id, r1);
    let tool = spans
        .iter()
        .find(|s| s.name.starts_with(TOOL_SPAN))
        .expect("tool");
    assert_eq!(tool.parent_span_id, r2);
    // Why: a request without a gateway trace id still gets a stable trace of its
    // own.
    let r2_span = spans.iter().find(|s| s.span_id == r2).expect("r2");
    assert_eq!(r2_span.trace_id, trace_id_bytes("r2"));
    assert_eq!(r2_span.status.as_ref().map(|s| s.code), Some(STATUS_ERROR));
    assert_eq!(
        r2_span.status.as_ref().map(|s| s.message.as_str()),
        Some("upstream 500")
    );
}

#[test]
fn log_records_carry_level_body_and_trace_correlation() {
    let row = LogRow {
        id: "log-1".to_owned(),
        timestamp: at(3),
        level: "WARN".to_owned(),
        module: "gateway".to_owned(),
        message: "slow upstream".to_owned(),
        metadata: Some("{\"ms\":900}".to_owned()),
        user_id: Some(UserId::new("user-1")),
        session_id: None,
        trace_id: Some(TraceId::new("trace-1")),
        context_id: None,
        client_id: None,
        instance_id: Some("node-a".to_owned()),
        provider_request_id: Some("r1".to_owned()),
        gateway_conversation_id: None,
    };
    let record = to_log_record(&row);
    assert_eq!(record.severity_text, "WARN");
    assert_eq!(record.severity_number, severity_number("WARN") as i32);
    assert_eq!(record.time_unix_nano, unix_nanos(at(3)));
    assert_eq!(record.trace_id, trace_id_bytes("trace-1"));
    assert_eq!(record.span_id, span_id_bytes(REQUEST_SPAN, "r1"));
    let body = record.body.and_then(|b| b.value);
    assert!(matches!(body, Some(Value::StringValue(s)) if s == "slow upstream"));

    let untraced = to_log_record(&LogRow {
        trace_id: None,
        ..row
    });
    assert!(untraced.trace_id.is_empty());
    assert!(untraced.span_id.is_empty());
}

#[test]
fn severity_follows_the_logs_level_vocabulary() {
    let ordered = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].map(|l| severity_number(l) as i32);
    assert!(ordered.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(severity_number("FATAL") as i32, 0);
}

#[test]
fn retry_policy_backs_off_and_only_retries_transient_failures() {
    assert!(RETRY_DELAYS.windows(2).all(|w| w[0] < w[1]));
    assert!(is_retryable(None));
    assert!(is_retryable(Some(reqwest::StatusCode::TOO_MANY_REQUESTS)));
    assert!(is_retryable(Some(reqwest::StatusCode::SERVICE_UNAVAILABLE)));
    assert!(!is_retryable(Some(reqwest::StatusCode::UNAUTHORIZED)));
    assert!(!is_retryable(Some(reqwest::StatusCode::BAD_REQUEST)));
    assert!(!is_retryable(Some(reqwest::StatusCode::NOT_FOUND)));
}

#[tokio::test]
async fn state_inventory_reports_each_signal_failure_and_acknowledged_progress() {
    let db = DisposableDb::installed("otlp_state_inventory")
        .await
        .unwrap();
    let pool = db.pool().await.unwrap();
    let raw = pool.pool_arc().expect("raw pool");
    let repository = systemprompt_scheduler::OtlpExportStateRepository::new(raw.as_ref().clone());

    repository
        .get_or_start(OtlpSignal::Traces)
        .await
        .expect("start traces cursor");
    repository
        .get_or_start(OtlpSignal::Logs)
        .await
        .expect("start logs cursor");
    repository
        .advance(OtlpSignal::Logs, &Watermark::new(at(7), "log-7"), 4)
        .await
        .expect("acknowledge logs batch");
    repository
        .record_failure(OtlpSignal::Traces, "collector unavailable")
        .await
        .expect("record traces failure");

    let states = repository.list_states().await.expect("list durable state");
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].signal, "logs");
    assert_eq!(states[0].watermark_id, "log-7");
    assert_eq!(states[0].batches_total, 1);
    assert_eq!(states[0].rows_total, 4);
    assert_eq!(states[0].failures_total, 0);
    assert!(states[0].last_success_at.is_some());
    assert!(states[0].last_error.is_none());
    assert_eq!(states[1].signal, "traces");
    assert_eq!(states[1].watermark_id, "");
    assert_eq!(states[1].batches_total, 0);
    assert_eq!(states[1].rows_total, 0);
    assert_eq!(states[1].failures_total, 1);
    assert_eq!(
        states[1].last_error.as_deref(),
        Some("collector unavailable")
    );
    assert!(states[1].last_error_at.is_some());

    drop(repository);
    raw.close().await;
    drop(raw);
    drop(pool);
    db.drop_now().await;
}
