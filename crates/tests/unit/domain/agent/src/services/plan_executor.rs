use serde_json::Value;
use systemprompt_agent::services::a2a_server::processing::strategies::plan_executor::{
    ToolExecutorTrait, ToolOutcome, convert_to_call_tool_results, convert_to_tool_calls,
    execute_tools, format_results_for_response,
};
use systemprompt_agent::services::shared::Result;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, McpToolName, SessionId, TraceId, UserId,
};
use systemprompt_models::McpTool;
use systemprompt_models::ai::{ExecutionState, PlannedToolCall, PlannedToolResult};
use systemprompt_models::execution::context::RequestContext;

struct AlwaysOkExecutor;
#[async_trait::async_trait]
impl ToolExecutorTrait for AlwaysOkExecutor {
    async fn execute_tool(
        &self,
        tool_name: &McpToolName,
        arguments: Value,
        _tools: &[McpTool],
        _ctx: &RequestContext,
    ) -> Result<ToolOutcome> {
        Ok(ToolOutcome {
            output: serde_json::json!({"echo_tool": tool_name, "args": arguments}),
            meta: Some(serde_json::json!({
                "io.systemprompt/execution": {"artifact_id": "art-1", "mcp_execution_id": "exec-1"}
            })),
        })
    }
}

struct AlwaysFailExecutor;
#[async_trait::async_trait]
impl ToolExecutorTrait for AlwaysFailExecutor {
    async fn execute_tool(
        &self,
        _tool_name: &McpToolName,
        _arguments: Value,
        _tools: &[McpTool],
        _ctx: &RequestContext,
    ) -> Result<ToolOutcome> {
        Err(systemprompt_agent::services::shared::AgentServiceError::Internal("boom".to_string()))
    }
}

fn ctx() -> RequestContext {
    let mut c = RequestContext::new(
        SessionId::new("pe-session"),
        TraceId::new("pe-trace"),
        ContextId::generate(),
        AgentName::try_new("pe-agent").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    );
    c.auth.actor = Actor::user(UserId::new("pe-user"));
    c
}

fn call(name: &str) -> PlannedToolCall {
    PlannedToolCall::new(name, serde_json::json!({"x": 1}))
}

#[test]
fn convert_to_tool_calls_assigns_unique_ids() {
    let calls = vec![call("a"), call("b"), call("c")];
    let tool_calls = convert_to_tool_calls(&calls);
    assert_eq!(tool_calls.len(), 3);
    assert_eq!(tool_calls[0].name, "a");
    let ids: std::collections::HashSet<&str> = tool_calls
        .iter()
        .map(|c| c.ai_tool_call_id.as_str())
        .collect();
    assert_eq!(ids.len(), 3, "each planned call gets a unique id");
    assert!(ids.iter().all(|id| !id.is_empty()));
}

#[test]
fn convert_to_tool_calls_empty() {
    let r = convert_to_tool_calls(&[]);
    assert!(r.is_empty());
}

#[test]
fn convert_to_call_tool_results_maps_success_and_failure() {
    let mut state = ExecutionState::new();
    state.add_result(PlannedToolResult::success(
        "ok_tool".to_string(),
        serde_json::json!({}),
        serde_json::json!({"out": "ok"}),
        10,
    ));
    state.add_result(PlannedToolResult::failure(
        "bad_tool".to_string(),
        serde_json::json!({}),
        "fail reason".to_string(),
        20,
    ));
    let results = convert_to_call_tool_results(&state);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].is_error, Some(false));
    assert_eq!(results[1].is_error, Some(true));
}

#[tokio::test]
async fn executed_tools_keep_the_wire_meta_for_the_artifact_transformer() {
    let calls = vec![call("with_meta")];
    let state = execute_tools(&calls, &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    let results = convert_to_call_tool_results(&state);
    let meta = results[0]
        .meta
        .as_ref()
        .expect("meta survives reconstruction");
    assert!(
        meta.0.contains_key("io.systemprompt/execution"),
        "the execution meta key reaches the artifact transformer"
    );
}

#[test]
fn format_results_for_response_includes_indices_and_status() {
    let mut state = ExecutionState::new();
    state.add_result(PlannedToolResult::success(
        "first".to_string(),
        serde_json::json!({}),
        serde_json::json!({"answer": 42}),
        5,
    ));
    state.add_result(PlannedToolResult::failure(
        "second".to_string(),
        serde_json::json!({}),
        "oops".to_string(),
        6,
    ));
    let summary = format_results_for_response(&state);
    assert!(summary.contains("1. first - SUCCESS"));
    assert!(summary.contains("2. second - FAILED"));
    assert!(summary.contains("oops"));
}

#[test]
fn format_results_for_response_empty_state() {
    let state = ExecutionState::new();
    let summary = format_results_for_response(&state);
    assert_eq!(summary, "");
}

#[tokio::test]
async fn execute_tools_collects_results() {
    let calls = vec![call("alpha"), call("beta")];
    let state = execute_tools(&calls, &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 2);
    assert_eq!(state.successful_results().len(), 2);
    assert!(state.failed_results().is_empty());
}

#[tokio::test]
async fn execute_tools_records_failures() {
    let calls = vec![call("x")];
    let state = execute_tools(&calls, &[], &ctx(), &AlwaysFailExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 1);
    assert_eq!(state.failed_results().len(), 1);
    assert!(
        state.failed_results()[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("boom")
    );
}

#[tokio::test]
async fn execute_tools_empty_calls_returns_empty_state() {
    let state = execute_tools(&[], &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert!(state.results.is_empty());
}

#[tokio::test]
async fn execute_tools_without_templates_runs_plainly() {
    let calls = vec![call("plain")];
    let state = execute_tools(&calls, &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 1);
    assert!(state.results[0].success);
}

#[tokio::test]
async fn a_planned_call_without_a_tool_name_fails_before_reaching_the_executor() {
    let calls = vec![call("")];
    let state = execute_tools(&calls, &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 1);
    assert_eq!(state.failed_results().len(), 1);
    assert!(
        state.failed_results()[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("tool_name"),
        "the failure names the rejected field"
    );
}
