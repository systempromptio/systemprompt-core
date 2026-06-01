use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::McpResponseBuilder;
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::cli::CliArtifact;
use systemprompt_models::artifacts::{DashboardArtifact, ListArtifact, TextArtifact};

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("session-1"),
        TraceId::new("trace-1"),
        ContextId::new("00000000-0000-4000-8000-000000000002"),
        AgentName::new("test-agent"),
    )
}

#[test]
fn build_error_is_marked_as_error() {
    let r = McpResponseBuilder::<TextArtifact>::build_error("fail");
    assert_eq!(r.is_error, Some(true));
}

#[test]
fn build_error_has_text_content() {
    let r = McpResponseBuilder::<TextArtifact>::build_error("fail message");
    assert!(!r.content.is_empty());
    let serialized = serde_json::to_string(&r.content).expect("serialize");
    assert!(serialized.contains("fail message"));
}

#[test]
fn build_error_with_newlines() {
    let r = McpResponseBuilder::<TextArtifact>::build_error("line1\nline2\nline3");
    let serialized = serde_json::to_string(&r.content).expect("serialize");
    assert!(serialized.contains("line1"));
}

#[test]
fn builder_new_exec_id_in_debug() {
    let c = ctx();
    let exec_id = McpExecutionId::generate();
    let exec_id_str = exec_id.to_string();
    let t = TextArtifact::new("data", &c);
    let builder = McpResponseBuilder::new(t, "tool-x", &c, &exec_id);
    let debug = format!("{builder:?}");
    assert!(debug.contains(&exec_id_str));
}

#[test]
fn build_error_for_list_type() {
    let r = McpResponseBuilder::<ListArtifact>::build_error("list error");
    assert_eq!(r.is_error, Some(true));
}

#[test]
fn build_error_for_dashboard_type() {
    let r = McpResponseBuilder::<DashboardArtifact>::build_error("dash error");
    assert_eq!(r.is_error, Some(true));
}

#[test]
fn build_error_for_cli_artifact_type() {
    let r = McpResponseBuilder::<CliArtifact>::build_error("cli error");
    assert_eq!(r.is_error, Some(true));
}

#[test]
fn builder_debug_contains_tool_name() {
    let c = ctx();
    let exec_id = McpExecutionId::generate();
    let t = TextArtifact::new("payload", &c);
    let builder = McpResponseBuilder::new(t, "my-custom-tool", &c, &exec_id);
    let debug = format!("{builder:?}");
    assert!(debug.contains("my-custom-tool"));
}

#[test]
fn build_error_no_structured_content() {
    let r = McpResponseBuilder::<TextArtifact>::build_error("err");
    assert!(r.structured_content.is_none());
}

#[test]
fn build_error_multiple_calls_independent() {
    let r1 = McpResponseBuilder::<TextArtifact>::build_error("first");
    let r2 = McpResponseBuilder::<TextArtifact>::build_error("second");
    let s1 = serde_json::to_string(&r1.content).expect("s1");
    let s2 = serde_json::to_string(&r2.content).expect("s2");
    assert!(s1.contains("first"));
    assert!(s2.contains("second"));
    assert!(!s1.contains("second"));
}

#[test]
fn build_error_empty_string_message() {
    let r = McpResponseBuilder::<TextArtifact>::build_error("");
    assert_eq!(r.is_error, Some(true));
}

#[test]
fn builder_new_with_owned_string_tool() {
    let c = ctx();
    let exec_id = McpExecutionId::generate();
    let t = TextArtifact::new("val", &c);
    let tool_name = "dynamic-tool".to_owned();
    let builder = McpResponseBuilder::new(t, tool_name, &c, &exec_id);
    let debug = format!("{builder:?}");
    assert!(debug.contains("dynamic-tool"));
}
