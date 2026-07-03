//! Unit tests for ToolResultHandler::process_tool_result.
//!
//! Target: crates/domain/agent/src/services/mcp/tool_result_handler.rs
//!
//! Covers the auth gate (unauthenticated and system callers are rejected) and
//! the success path where an authenticated user's tool result is transformed
//! into a validated A2A artifact.

use rmcp::model::CallToolResult;
use serde_json::json;
use systemprompt_agent::services::mcp::tool_result_handler::{
    ProcessToolResultParams, ToolResultHandler,
};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId};
use systemprompt_models::auth::{AuthenticatedUser, Permission, UserType};
use systemprompt_models::execution::context::RequestContext;
use uuid::Uuid;

const CTX_UUID: &str = "00000000-0000-4000-8000-000000000001";

fn authed_user() -> AuthenticatedUser {
    AuthenticatedUser::new(
        Uuid::new_v4(),
        "tool-user".to_owned(),
        "tool@example.com".to_owned(),
        vec![Permission::Admin],
    )
}

fn base_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("trh-session"),
        TraceId::new("trh-trace"),
        ContextId::new(CTX_UUID),
        AgentName::new("trh-agent"),
    )
}

fn valid_tool_result() -> CallToolResult {
    serde_json::from_value(json!({
        "structuredContent": {
            "artifact_id": "art-trh-1",
            "mcp_execution_id": "exec-trh-1",
            "artifact": {"x-artifact-type": "text", "value": "hello"},
            "_metadata": {"skill_id": "skill-1", "skill_name": "writer"}
        }
    }))
    .expect("build CallToolResult")
}

#[test]
fn rejects_unauthenticated_caller() {
    let ctx = base_ctx();
    let tr = valid_tool_result();
    let task_id = TaskId::new("task-trh-1");
    let context_id = ContextId::new(CTX_UUID);
    let args = json!({});
    let p = ProcessToolResultParams {
        tool_name: "writer-tool",
        tool_result: &tr,
        output_schema: None,
        tool_arguments: Some(&args),
        task_id: &task_id,
        context_id: &context_id,
        context: &ctx,
    };

    let err = ToolResultHandler::process_tool_result(&p).expect_err("unauthenticated rejected");
    assert!(err.to_string().contains("Invalid user"));
}

#[test]
fn rejects_system_caller() {
    let ctx = base_ctx()
        .with_user(authed_user())
        .with_user_type(UserType::Service);
    let tr = valid_tool_result();
    let task_id = TaskId::new("task-trh-2");
    let context_id = ContextId::new(CTX_UUID);
    let args = json!({});
    let p = ProcessToolResultParams {
        tool_name: "writer-tool",
        tool_result: &tr,
        output_schema: None,
        tool_arguments: Some(&args),
        task_id: &task_id,
        context_id: &context_id,
        context: &ctx,
    };

    let err = ToolResultHandler::process_tool_result(&p).expect_err("system caller rejected");
    assert!(err.to_string().contains("Invalid user"));
}

#[test]
fn transforms_authenticated_tool_result() {
    let ctx = base_ctx().with_user(authed_user());
    let tr = valid_tool_result();
    let task_id = TaskId::new("task-trh-3");
    let context_id = ContextId::new(CTX_UUID);
    let args = json!({"q": 1});
    let p = ProcessToolResultParams {
        tool_name: "writer-tool",
        tool_result: &tr,
        output_schema: None,
        tool_arguments: Some(&args),
        task_id: &task_id,
        context_id: &context_id,
        context: &ctx,
    };

    let artifact = ToolResultHandler::process_tool_result(&p).expect("transform succeeds");
    assert_eq!(artifact.id.as_str(), "art-trh-1");
    assert_eq!(artifact.title.as_deref(), Some("writer-tool"));
    assert_eq!(artifact.metadata.artifact_type, "text");
    assert_eq!(
        artifact.metadata.mcp_execution_id.as_deref(),
        Some("exec-trh-1")
    );
}

#[test]
fn missing_structured_content_errors() {
    let ctx = base_ctx().with_user(authed_user());
    let tr: CallToolResult = serde_json::from_value(json!({"content": []})).expect("build");
    let task_id = TaskId::new("task-trh-4");
    let context_id = ContextId::new(CTX_UUID);
    let args = json!({});
    let p = ProcessToolResultParams {
        tool_name: "writer-tool",
        tool_result: &tr,
        output_schema: None,
        tool_arguments: Some(&args),
        task_id: &task_id,
        context_id: &context_id,
        context: &ctx,
    };

    let err = ToolResultHandler::process_tool_result(&p).expect_err("no structured content");
    assert!(err.to_string().to_lowercase().contains("transform"));
}

#[test]
fn default_handler_constructs() {
    let handler = ToolResultHandler::default();
    assert!(format!("{handler:?}").contains("ToolResultHandler"));
}
