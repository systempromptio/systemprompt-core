//! The audit spans of the live OTLP export pin the attribute set the
//! audit-trail replay emitted: `ai_request`, `tool_call` and
//! `governance_decision`, their names, parents and status.

use chrono::{Duration, TimeZone, Utc};
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::trace::v1::Span;
use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::status::StatusCode;
use systemprompt_identifiers::{
    AiRequestId, AiToolCallId, ApiKeyId, ContextId, InstanceId, McpExecutionId, McpServerId,
    McpToolName, PluginId, ScopeDimension, SessionId, TraceId, UserId,
};
use systemprompt_logging::otlp::{
    GOVERNANCE_SPAN, REQUEST_SPAN, TOOL_SPAN, governance_span, request_span, span_id_bytes,
    tool_call_span, trace_id_bytes,
};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource};
use systemprompt_models::audit_spans::{
    GovernanceSpanRecord, RequestSpanRecord, ToolCallSpanRecord,
};
use systemprompt_traits::RequestStatus;

fn keys(span: &Span) -> Vec<&str> {
    span.attributes.iter().map(|kv| kv.key.as_str()).collect()
}

fn value<'a>(span: &'a Span, key: &str) -> &'a Value {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| kv.value.as_ref())
        .and_then(|v| v.value.as_ref())
        .unwrap_or_else(|| panic!("attribute {key} missing"))
}

fn text<'a>(span: &'a Span, key: &str) -> &'a str {
    match value(span, key) {
        Value::StringValue(s) => s,
        other => panic!("{key} is not a string: {other:?}"),
    }
}

fn int(span: &Span, key: &str) -> i64 {
    match value(span, key) {
        Value::IntValue(n) => *n,
        other => panic!("{key} is not an int: {other:?}"),
    }
}

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap()
}

fn request() -> RequestSpanRecord {
    RequestSpanRecord {
        request_id: AiRequestId::new("req-1"),
        request_kind: "chat".to_owned(),
        status: RequestStatus::Completed,
        error_message: None,
        user_id: UserId::new("user-1"),
        actor_kind: "user".to_owned(),
        actor_id: "user-1".to_owned(),
        session_id: Some(SessionId::new("sess-1")),
        context_id: ContextId::generate(),
        trace_id: Some(TraceId::new("trace-1")),
        provider: Some("anthropic".to_owned()),
        served_provider: Some("bedrock".to_owned()),
        requested_model: Some("claude-x".to_owned()),
        model: Some("claude-x-served".to_owned()),
        route_match: Some("route:default".to_owned()),
        input_tokens: Some(10),
        output_tokens: Some(20),
        cache_read_tokens: Some(3),
        cache_creation_tokens: Some(4),
        cost_microdollars: 1_500_000,
        latency_ms: Some(900),
        upstream_latency_ms: Some(800),
        finish_reason: Some("end_turn".to_owned()),
        client_kind: "claude_code".to_owned(),
        wire_protocol: "anthropic".to_owned(),
        instance_id: Some(InstanceId::new("inst-1")),
        api_key_id: Some(ApiKeyId::new("key-1")),
        attributions: vec![AttributionEntry {
            dimension: ScopeDimension::try_new("team").expect("valid dimension"),
            value: "platform".to_owned(),
            source: AttributionSource::Header,
        }],
        started_at: at(),
        completed_at: at() + Duration::milliseconds(900),
    }
}

#[test]
fn request_span_carries_the_replay_attribute_set() {
    let span = request_span(&request());
    assert_eq!(
        keys(&span),
        vec![
            "systemprompt.request.id",
            "systemprompt.request.kind",
            "systemprompt.request.status",
            "enduser.id",
            "systemprompt.actor.kind",
            "systemprompt.actor.id",
            "session.id",
            "systemprompt.context.id",
            "systemprompt.trace.id",
            "gen_ai.system",
            "systemprompt.served_provider",
            "gen_ai.request.model",
            "gen_ai.response.model",
            "systemprompt.route",
            "gen_ai.usage.input_tokens",
            "gen_ai.usage.output_tokens",
            "gen_ai.usage.cache_read_tokens",
            "gen_ai.usage.cache_creation_tokens",
            "systemprompt.cost.usd",
            "systemprompt.cost.microdollars",
            "systemprompt.latency_ms",
            "systemprompt.upstream_latency_ms",
            "gen_ai.response.finish_reason",
            "systemprompt.client.kind",
            "systemprompt.wire_protocol",
            "systemprompt.instance.id",
            "systemprompt.api_key.id",
            "systemprompt.scope.team",
            "systemprompt.scope.team.source",
        ]
    );
    assert_eq!(text(&span, "systemprompt.request.status"), "completed");
    assert_eq!(text(&span, "systemprompt.api_key.id"), "key-1");
    assert_eq!(text(&span, "systemprompt.scope.team"), "platform");
    assert_eq!(text(&span, "systemprompt.scope.team.source"), "header");
    assert_eq!(int(&span, "systemprompt.cost.microdollars"), 1_500_000);
    assert!(
        matches!(value(&span, "systemprompt.cost.usd"), Value::DoubleValue(v) if (*v - 1.5).abs() < f64::EPSILON)
    );
    assert_eq!(span.name, format!("{REQUEST_SPAN} claude-x-served"));
    assert_eq!(span.kind, SpanKind::Server as i32);
    assert_eq!(span.trace_id, trace_id_bytes("trace-1"));
    assert_eq!(span.span_id, span_id_bytes(REQUEST_SPAN, "req-1"));
    assert!(span.parent_span_id.is_empty());
    assert_eq!(
        span.status.as_ref().map(|s| s.code),
        Some(StatusCode::Ok as i32)
    );
}

#[test]
fn failed_request_span_is_an_error_with_its_message() {
    let mut record = request();
    record.status = RequestStatus::Failed;
    record.error_message = Some("upstream refused".to_owned());
    let span = request_span(&record);
    let status = span.status.clone().expect("status");
    assert_eq!(status.code, StatusCode::Error as i32);
    assert_eq!(status.message, "upstream refused");
    assert_eq!(text(&span, "systemprompt.request.status"), "failed");
}

fn tool_call() -> ToolCallSpanRecord {
    ToolCallSpanRecord {
        ai_tool_call_id: Some(AiToolCallId::new("toolu_1")),
        request_id: Some(AiRequestId::new("req-1")),
        trace_id: Some(TraceId::new("trace-1")),
        mcp_execution_id: Some(McpExecutionId::new("exec-1")),
        tool_name: Some(McpToolName::new("search")),
        server_name: Some(McpServerId::new("docs")),
        state: Some("executed".to_owned()),
        source: Some("in_process".to_owned()),
        execution_status: Some("success".to_owned()),
        execution_time_ms: Some(42),
        error_message: None,
        artifact_type: Some("text".to_owned()),
        payload_bytes: Some(128),
        secret_redactions: Some(0),
        is_error: false,
        started_at: at(),
        completed_at: at() + Duration::milliseconds(42),
    }
}

#[test]
fn tool_call_span_carries_the_replay_attribute_set() {
    let span = tool_call_span(&tool_call());
    assert_eq!(
        keys(&span),
        vec![
            "gen_ai.tool.name",
            "gen_ai.tool.call.id",
            "systemprompt.mcp.server",
            "systemprompt.mcp.execution_id",
            "systemprompt.tool.state",
            "systemprompt.tool.source",
            "systemprompt.tool.status",
            "systemprompt.tool.execution_ms",
            "systemprompt.artifact.type",
            "systemprompt.artifact.bytes",
            "systemprompt.artifact.secret_redactions",
            "systemprompt.artifact.is_error",
        ]
    );
    assert_eq!(text(&span, "gen_ai.tool.name"), "search");
    assert_eq!(int(&span, "systemprompt.tool.execution_ms"), 42);
    assert_eq!(span.name, format!("{TOOL_SPAN} search"));
    assert_eq!(span.span_id, span_id_bytes(TOOL_SPAN, "toolu_1"));
    assert_eq!(span.parent_span_id, span_id_bytes(REQUEST_SPAN, "req-1"));
    assert_eq!(span.trace_id, trace_id_bytes("trace-1"));
}

#[test]
fn tool_call_span_without_a_known_request_is_a_root_of_its_trace() {
    let mut record = tool_call();
    record.request_id = None;
    record.execution_status = Some("failed".to_owned());
    let span = tool_call_span(&record);
    assert!(span.parent_span_id.is_empty());
    assert_eq!(span.trace_id, trace_id_bytes("trace-1"));
    assert_eq!(span.status.map(|s| s.code), Some(StatusCode::Error as i32));
}

#[test]
fn governance_span_carries_the_replay_attribute_set() {
    let span = governance_span(&GovernanceSpanRecord {
        id: "gd-1".to_owned(),
        request_id: None,
        trace_id: Some(TraceId::new("trace-1")),
        decision: "deny".to_owned(),
        policy: "rule_based".to_owned(),
        reason: "no grant".to_owned(),
        tool_name: "mcp__docs__search".to_owned(),
        tool_use_id: Some("toolu_1".to_owned()),
        plugin_id: Some(PluginId::new("plugin-1")),
        actor_kind: "user".to_owned(),
        actor_id: "user-1".to_owned(),
        decided_at: at(),
    });
    assert_eq!(
        keys(&span),
        vec![
            "systemprompt.governance.decision",
            "systemprompt.governance.policy",
            "systemprompt.governance.reason",
            "gen_ai.tool.name",
            "gen_ai.tool.call.id",
            "systemprompt.plugin.id",
            "systemprompt.actor.kind",
            "systemprompt.actor.id",
        ]
    );
    assert_eq!(span.name, format!("{GOVERNANCE_SPAN} rule_based"));
    assert_eq!(span.span_id, span_id_bytes(GOVERNANCE_SPAN, "gd-1"));
    assert_eq!(span.trace_id, trace_id_bytes("trace-1"));
    let status = span.status.expect("status");
    assert_eq!(status.code, StatusCode::Error as i32);
    assert_eq!(status.message, "no grant");
}

#[test]
fn audit_span_attributes_carry_no_secret_material() {
    let spans = [request_span(&request()), tool_call_span(&tool_call())];
    for span in &spans {
        for key in keys(span) {
            assert!(
                !(key.contains("secret") && !key.ends_with("secret_redactions"))
                    && !key.contains("token.value")
                    && !key.contains("authorization")
                    && !key.contains("password"),
                "{key} looks like secret material"
            );
        }
    }
}
