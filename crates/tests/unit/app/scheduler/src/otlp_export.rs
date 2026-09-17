//! The pure half of the `otlp_export` job: cursor arithmetic, pacing, id
//! derivation, and the audit-row → OTLP conversion. Nothing here touches the
//! database or the network.

use chrono::{DateTime, Duration, TimeZone, Utc};
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::trace::v1::Span;
use systemprompt_identifiers::{ContextId, SessionId, TraceId, UserId};
use systemprompt_scheduler::jobs::otlp_export::{
    GOVERNANCE_SPAN, GovernanceRow, LedgerRow, LogRow, OtlpExportJob, REQUEST_SPAN, RETRY_DELAYS,
    RequestRow, TOOL_SPAN, TraceBatch, Watermark, is_retryable, pacing_elapsed, severity_number,
    span_id_bytes, to_log_record, to_spans, trace_id_bytes, unix_nanos,
};
use systemprompt_traits::Job;

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
