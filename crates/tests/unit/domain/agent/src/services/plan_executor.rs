use serde_json::Value;
use systemprompt_agent::services::a2a_server::processing::strategies::plan_executor::{
    ToolExecutorTrait, convert_to_call_tool_results, convert_to_tool_calls, execute_tools_sequentially,
    execute_tools_with_templates, format_results_for_response,
};
use systemprompt_agent::services::shared::Result;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::ai::{ExecutionState, PlannedToolCall, ToolCallResult};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::McpTool;

struct AlwaysOkExecutor;
#[async_trait::async_trait]
impl ToolExecutorTrait for AlwaysOkExecutor {
    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        _tools: &[McpTool],
        _ctx: &RequestContext,
    ) -> Result<Value> {
        Ok(serde_json::json!({"echo_tool": tool_name, "args": arguments}))
    }
}

struct AlwaysFailExecutor;
#[async_trait::async_trait]
impl ToolExecutorTrait for AlwaysFailExecutor {
    async fn execute_tool(
        &self,
        _tool_name: &str,
        _arguments: Value,
        _tools: &[McpTool],
        _ctx: &RequestContext,
    ) -> Result<Value> {
        Err(
            systemprompt_agent::services::shared::AgentServiceError::Internal(
                "boom".to_string(),
            ),
        )
    }
}

fn ctx() -> RequestContext {
    let mut c = RequestContext::new(
        SessionId::new("pe-session"),
        TraceId::new("pe-trace"),
        ContextId::generate(),
        AgentName::new("pe-agent"),
    );
    c.auth.actor = Actor::user(UserId::new("pe-user"));
    c
}

fn call(name: &str) -> PlannedToolCall {
    PlannedToolCall::new(name, serde_json::json!({"x": 1}))
}

#[test]
fn convert_to_tool_calls_assigns_ids() {
    let calls = vec![call("a"), call("b"), call("c")];
    let tool_calls = convert_to_tool_calls(&calls);
    assert_eq!(tool_calls.len(), 3);
    assert_eq!(tool_calls[0].name, "a");
    assert_eq!(tool_calls[0].ai_tool_call_id.as_str(), "plan_call_0");
    assert_eq!(tool_calls[1].ai_tool_call_id.as_str(), "plan_call_1");
    assert_eq!(tool_calls[2].ai_tool_call_id.as_str(), "plan_call_2");
}

#[test]
fn convert_to_tool_calls_empty() {
    let r = convert_to_tool_calls(&[]);
    assert!(r.is_empty());
}

#[test]
fn convert_to_call_tool_results_maps_success_and_failure() {
    let mut state = ExecutionState::new();
    state.add_result(ToolCallResult::success(
        "ok_tool".to_string(),
        serde_json::json!({}),
        serde_json::json!({"out": "ok"}),
        10,
    ));
    state.add_result(ToolCallResult::failure(
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

#[test]
fn format_results_for_response_includes_indices_and_status() {
    let mut state = ExecutionState::new();
    state.add_result(ToolCallResult::success(
        "first".to_string(),
        serde_json::json!({}),
        serde_json::json!({"answer": 42}),
        5,
    ));
    state.add_result(ToolCallResult::failure(
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
async fn execute_tools_sequentially_collects_results() {
    let calls = vec![call("alpha"), call("beta")];
    let state = execute_tools_sequentially(&calls, &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 2);
    assert_eq!(state.successful_results().len(), 2);
    assert!(state.failed_results().is_empty());
}

#[tokio::test]
async fn execute_tools_sequentially_records_failures() {
    let calls = vec![call("x")];
    let state = execute_tools_sequentially(&calls, &[], &ctx(), &AlwaysFailExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 1);
    assert_eq!(state.failed_results().len(), 1);
    assert!(state.failed_results()[0]
        .error
        .as_deref()
        .unwrap_or("")
        .contains("boom"));
}

#[tokio::test]
async fn execute_tools_sequentially_empty_calls_returns_empty_state() {
    let state = execute_tools_sequentially(&[], &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert!(state.results.is_empty());
}

#[tokio::test]
async fn execute_tools_with_templates_no_templates_runs_like_sequential() {
    let calls = vec![call("plain")];
    let state = execute_tools_with_templates(&calls, &[], &ctx(), &AlwaysOkExecutor)
        .await
        .expect("ok");
    assert_eq!(state.results.len(), 1);
    assert!(state.results[0].success);
}
