//! Tests for log-search and summary DTOs: LevelCount, ModuleCount,
//! LogTimeRange, LogSearchItem, and the audit/tool linkage rows.

use chrono::Utc;
use systemprompt_logging::trace::{
    AuditLookupResult, AuditToolCallRow, LevelCount, LinkedMcpCall, LogSearchItem, LogTimeRange,
    ModuleCount, ToolExecutionItem,
};

#[test]
fn level_count_construction() {
    let lc = LevelCount {
        level: "ERROR".to_owned(),
        count: 42,
    };
    assert_eq!(lc.level, "ERROR");
    assert_eq!(lc.count, 42);
}

#[test]
fn level_count_clone_and_debug() {
    let lc = LevelCount {
        level: "WARN".to_owned(),
        count: 10,
    };
    let cloned = lc.clone();
    assert_eq!(cloned.level, lc.level);
    assert!(format!("{lc:?}").contains("LevelCount"));
}

#[test]
fn level_count_serialize_roundtrip() {
    let lc = LevelCount {
        level: "INFO".to_owned(),
        count: 100,
    };
    let json = serde_json::to_string(&lc).unwrap();
    let back: LevelCount = serde_json::from_str(&json).unwrap();
    assert_eq!(back.level, "INFO");
    assert_eq!(back.count, 100);
}

#[test]
fn module_count_construction() {
    let mc = ModuleCount {
        module: "auth::login".to_owned(),
        count: 55,
    };
    assert_eq!(mc.module, "auth::login");
    assert_eq!(mc.count, 55);
}

#[test]
fn module_count_clone_and_debug() {
    let mc = ModuleCount {
        module: "database".to_owned(),
        count: 7,
    };
    let cloned = mc.clone();
    assert_eq!(cloned.module, mc.module);
    assert!(format!("{mc:?}").contains("ModuleCount"));
}

#[test]
fn module_count_serialize_roundtrip() {
    let mc = ModuleCount {
        module: "api::handler".to_owned(),
        count: 3,
    };
    let json = serde_json::to_string(&mc).unwrap();
    let back: ModuleCount = serde_json::from_str(&json).unwrap();
    assert_eq!(back.module, "api::handler");
    assert_eq!(back.count, 3);
}

#[test]
fn log_time_range_both_present() {
    let now = Utc::now();
    let later = now + chrono::Duration::hours(1);
    let tr = LogTimeRange {
        earliest: Some(now),
        latest: Some(later),
    };
    assert_eq!(tr.earliest, Some(now));
    assert_eq!(tr.latest, Some(later));
}

#[test]
fn log_time_range_both_none() {
    let tr = LogTimeRange {
        earliest: None,
        latest: None,
    };
    assert!(tr.earliest.is_none());
    assert!(tr.latest.is_none());
}

#[test]
fn log_time_range_copy_and_serialize() {
    let tr = LogTimeRange {
        earliest: Some(Utc::now()),
        latest: None,
    };
    let copy = tr;
    assert_eq!(copy.earliest, tr.earliest);
    let json = serde_json::to_string(&tr).unwrap();
    let back: LogTimeRange = serde_json::from_str(&json).unwrap();
    assert!(back.latest.is_none());
}

#[test]
fn log_search_item_construction() {
    let item = LogSearchItem {
        id: systemprompt_identifiers::LogId::generate(),
        trace_id: systemprompt_identifiers::TraceId::new("trace-abc"),
        timestamp: Utc::now(),
        level: "ERROR".to_owned(),
        module: "auth".to_owned(),
        message: "login failed".to_owned(),
        metadata: Some(r#"{"ip":"1.2.3.4"}"#.to_owned()),
    };
    assert_eq!(item.level, "ERROR");
    assert_eq!(item.module, "auth");
    assert_eq!(item.metadata.as_deref(), Some(r#"{"ip":"1.2.3.4"}"#));
}

#[test]
fn log_search_item_no_metadata() {
    let item = LogSearchItem {
        id: systemprompt_identifiers::LogId::generate(),
        trace_id: systemprompt_identifiers::TraceId::new("trace-xyz"),
        timestamp: Utc::now(),
        level: "INFO".to_owned(),
        module: "db".to_owned(),
        message: "connected".to_owned(),
        metadata: None,
    };
    assert!(item.metadata.is_none());
}

#[test]
fn log_search_item_clone_and_serialize() {
    let item = LogSearchItem {
        id: systemprompt_identifiers::LogId::generate(),
        trace_id: systemprompt_identifiers::TraceId::new("t"),
        timestamp: Utc::now(),
        level: "WARN".to_owned(),
        module: "m".to_owned(),
        message: "msg".to_owned(),
        metadata: None,
    };
    let cloned = item.clone();
    assert_eq!(cloned.level, item.level);
    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains("WARN"));
}

#[test]
fn audit_lookup_result_construction() {
    let result = AuditLookupResult {
        id: "req-audit-1".to_owned().into(),
        provider: "anthropic".to_owned(),
        model: "claude-3".to_owned(),
        requested_model: Some("claude-3-opus".to_owned()),
        input_tokens: Some(200),
        output_tokens: Some(100),
        cost_microdollars: 5,
        latency_ms: Some(300),
        task_id: None,
        trace_id: None,
    };
    assert_eq!(result.provider, "anthropic");
    assert_eq!(result.cost_microdollars, 5);
    assert_eq!(result.requested_model.as_deref(), Some("claude-3-opus"));
    assert!(result.task_id.is_none());
}

#[test]
fn audit_lookup_result_with_ids() {
    let result = AuditLookupResult {
        id: "req-2".to_owned().into(),
        provider: "openai".to_owned(),
        model: "gpt-4".to_owned(),
        requested_model: None,
        input_tokens: None,
        output_tokens: None,
        cost_microdollars: 0,
        latency_ms: None,
        task_id: Some("task-abc".to_owned().into()),
        trace_id: Some(systemprompt_identifiers::TraceId::new("trace-def")),
    };
    assert_eq!(
        result.task_id.as_ref().map(|t| t.as_str()),
        Some("task-abc")
    );
    assert_eq!(
        result.trace_id.as_ref().map(|t| t.as_str()),
        Some("trace-def")
    );
}

#[test]
fn audit_lookup_result_serialize() {
    let result = AuditLookupResult {
        id: "req-ser".to_owned().into(),
        provider: "p".to_owned(),
        model: "m".to_owned(),
        requested_model: None,
        input_tokens: None,
        output_tokens: None,
        cost_microdollars: 1,
        latency_ms: None,
        task_id: None,
        trace_id: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("provider"));
}

#[test]
fn audit_tool_call_row_construction() {
    let row = AuditToolCallRow {
        tool_name: "read_file".to_owned(),
        tool_input: r#"{"path":"/tmp/x"}"#.to_owned(),
        sequence_number: 1,
    };
    assert_eq!(row.tool_name, "read_file");
    assert_eq!(row.sequence_number, 1);
}

#[test]
fn audit_tool_call_row_clone_and_serialize() {
    let row = AuditToolCallRow {
        tool_name: "write_file".to_owned(),
        tool_input: "{}".to_owned(),
        sequence_number: 2,
    };
    let cloned = row.clone();
    assert_eq!(cloned.tool_name, row.tool_name);
    let json = serde_json::to_string(&row).unwrap();
    assert!(json.contains("write_file"));
}

#[test]
fn linked_mcp_call_construction() {
    let call = LinkedMcpCall {
        tool_name: "search".to_owned(),
        server_name: "brave-search".to_owned(),
        status: "success".to_owned(),
        execution_time_ms: Some(120),
    };
    assert_eq!(call.tool_name, "search");
    assert_eq!(call.status, "success");
    assert_eq!(call.execution_time_ms, Some(120));
}

#[test]
fn linked_mcp_call_no_execution_time() {
    let call = LinkedMcpCall {
        tool_name: "tool".to_owned(),
        server_name: "srv".to_owned(),
        status: "pending".to_owned(),
        execution_time_ms: None,
    };
    assert!(call.execution_time_ms.is_none());
}

#[test]
fn linked_mcp_call_clone_and_serialize() {
    let call = LinkedMcpCall {
        tool_name: "t".to_owned(),
        server_name: "s".to_owned(),
        status: "ok".to_owned(),
        execution_time_ms: Some(50),
    };
    let cloned = call.clone();
    assert_eq!(cloned.server_name, call.server_name);
    let json = serde_json::to_string(&call).unwrap();
    assert!(json.contains("server_name"));
}

#[test]
fn tool_execution_item_construction() {
    let item = ToolExecutionItem {
        timestamp: Utc::now(),
        trace_id: systemprompt_identifiers::TraceId::new("t-trace"),
        tool_name: "bash".to_owned(),
        server_name: Some("docker-mcp".to_owned()),
        status: "success".to_owned(),
        execution_time_ms: Some(500),
    };
    assert_eq!(item.tool_name, "bash");
    assert_eq!(item.status, "success");
    assert_eq!(item.server_name.as_deref(), Some("docker-mcp"));
}

#[test]
fn tool_execution_item_no_server() {
    let item = ToolExecutionItem {
        timestamp: Utc::now(),
        trace_id: systemprompt_identifiers::TraceId::new("t2"),
        tool_name: "read".to_owned(),
        server_name: None,
        status: "pending".to_owned(),
        execution_time_ms: None,
    };
    assert!(item.server_name.is_none());
    assert!(item.execution_time_ms.is_none());
}

#[test]
fn tool_execution_item_clone_and_serialize() {
    let item = ToolExecutionItem {
        timestamp: Utc::now(),
        trace_id: systemprompt_identifiers::TraceId::new("t3"),
        tool_name: "list".to_owned(),
        server_name: None,
        status: "done".to_owned(),
        execution_time_ms: Some(10),
    };
    let cloned = item.clone();
    assert_eq!(cloned.tool_name, item.tool_name);
    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains("tool_name"));
}
