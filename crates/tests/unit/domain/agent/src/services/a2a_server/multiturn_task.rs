//! Unit tests for build_multiturn_task and its history/artifact builders.
//!
//! Target: crates/domain/agent/src/services/a2a_server/processing/task_builder/
//! builders.rs

use rmcp::model::{CallToolResult, ContentBlock};
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::processing::task_builder::{
    BuildMultiturnTaskParams, build_multiturn_task,
};
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, TaskId};
use systemprompt_models::ToolCall;

fn user_message(ctx: &ContextId, tid: &TaskId, text: &str) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: text.to_string(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(tid.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call-{name}")),
        name: name.to_string(),
        arguments: serde_json::json!({"k": "v"}),
    }
}

fn success_result(text: &str) -> CallToolResult {
    let mut r = CallToolResult::success(vec![ContentBlock::text(text.to_string())]);
    r.structured_content = Some(serde_json::json!({"out": text}));
    r
}

// A result whose structured_content is a valid A2A tool-response envelope, so
// build_artifacts produces an actual artifact (is_error = Some(false)).
fn artifact_result(artifact_id: &str, exec_id: &str) -> CallToolResult {
    let mut r = CallToolResult::success(vec![ContentBlock::text("tool output".to_string())]);
    r.structured_content = Some(serde_json::json!({
        "artifact_id": artifact_id,
        "mcp_execution_id": exec_id,
        "artifact": {"x-artifact-type": "text", "value": "v"},
        "_metadata": {}
    }));
    r
}

fn error_artifact_result(artifact_id: &str, exec_id: &str) -> CallToolResult {
    let mut r = CallToolResult::error(vec![ContentBlock::text("tool failed".to_string())]);
    r.structured_content = Some(serde_json::json!({
        "artifact_id": artifact_id,
        "mcp_execution_id": exec_id,
        "artifact": {"x-artifact-type": "text", "value": "v"},
        "_metadata": {}
    }));
    r
}

#[test]
fn build_multiturn_produces_artifact_for_valid_envelope() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "do"),
        tool_calls: vec![call("alpha")],
        tool_results: vec![artifact_result("art-mt-1", "exec-mt-1")],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    let artifacts = task.artifacts.expect("artifacts present");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].id.as_str(), "art-mt-1");
    assert_eq!(artifacts[0].metadata.artifact_type, "tool_execution");
    assert_eq!(artifacts[0].metadata.tool_name.as_deref(), Some("alpha"));
    assert_eq!(artifacts[0].title.as_deref(), Some("tool_execution_1"));
    // The data part records call_id, tool_name, output, status=success.
    match &artifacts[0].parts[0] {
        Part::Data(d) => {
            assert_eq!(d.data.get("status"), Some(&serde_json::json!("success")));
            assert_eq!(d.data.get("tool_name"), Some(&serde_json::json!("alpha")));
        },
        other => panic!("expected data part, got {other:?}"),
    }
}

#[test]
fn build_multiturn_marks_error_status_for_error_result() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "do"),
        tool_calls: vec![call("beta")],
        tool_results: vec![error_artifact_result("art-mt-2", "exec-mt-2")],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    let artifacts = task.artifacts.expect("artifacts present");
    assert_eq!(artifacts.len(), 1);
    match &artifacts[0].parts[0] {
        Part::Data(d) => {
            assert_eq!(d.data.get("status"), Some(&serde_json::json!("error")));
        },
        other => panic!("expected data part, got {other:?}"),
    }
}

#[test]
fn build_multiturn_skips_artifact_for_invalid_envelope() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    // structured_content present (is_error Some) but NOT a valid envelope, so
    // parse_tool_response fails and the artifact is skipped.
    let mut bad = CallToolResult::success(vec![ContentBlock::text("x".to_string())]);
    bad.structured_content = Some(serde_json::json!({"not": "an envelope"}));
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "do"),
        tool_calls: vec![call("gamma")],
        tool_results: vec![bad],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    assert!(task.artifacts.is_none());
}

#[test]
fn build_multiturn_two_artifacts_get_distinct_indices() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "do"),
        tool_calls: vec![call("a"), call("b")],
        tool_results: vec![
            artifact_result("art-x", "exec-x"),
            artifact_result("art-y", "exec-y"),
        ],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    let artifacts = task.artifacts.expect("artifacts");
    assert_eq!(artifacts.len(), 2);
    assert_eq!(artifacts[0].title.as_deref(), Some("tool_execution_1"));
    assert_eq!(artifacts[1].title.as_deref(), Some("tool_execution_2"));
}

#[test]
fn build_multiturn_no_tools_produces_user_and_final_history() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "hello"),
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "final answer".to_string(),
        total_iterations: 1,
    });
    assert_eq!(task.status.state, TaskState::Completed);
    let history = task.history.unwrap();
    // 2 entries: user + final synthesis.
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, MessageRole::User);
    assert_eq!(history[1].role, MessageRole::Agent);
}

#[test]
fn build_multiturn_with_tools_produces_three_extra_entries() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "do it"),
        tool_calls: vec![call("alpha"), call("beta")],
        tool_results: vec![success_result("a"), success_result("b")],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    let history = task.history.unwrap();
    // user + (executing + results) + final = 4 (single iteration consumed both
    // calls).
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].role, MessageRole::User);
    assert_eq!(history[1].role, MessageRole::Agent);
    assert_eq!(history[2].role, MessageRole::User);
    assert_eq!(history[3].role, MessageRole::Agent);
}

#[test]
fn build_multiturn_status_message_contains_final_response() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "x"),
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "wrapped-up".to_string(),
        total_iterations: 1,
    });
    let status_msg = task.status.message.unwrap();
    let txt = match &status_msg.parts[0] {
        Part::Text(t) => t.text.clone(),
        _ => panic!("expected text"),
    };
    assert_eq!(txt, "wrapped-up");
}

#[test]
fn build_multiturn_records_iteration_metadata() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "x"),
        tool_calls: vec![call("a")],
        tool_results: vec![success_result("r")],
        final_response: "done".to_string(),
        total_iterations: 7,
    });
    let metadata = task.metadata.unwrap();
    let ext = metadata.extensions;
    assert_eq!(ext.get("total_iterations"), Some(&serde_json::json!(7)));
    assert_eq!(ext.get("total_tools_called"), Some(&serde_json::json!(1)));
}

#[test]
fn build_multiturn_no_artifacts_when_no_structured_content() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let mut result_no_struct = CallToolResult::success(vec![ContentBlock::text("plain")]);
    result_no_struct.structured_content = None;
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "q"),
        tool_calls: vec![call("a")],
        tool_results: vec![result_no_struct],
        final_response: "x".to_string(),
        total_iterations: 1,
    });
    // structured_content::is_error is None, so build_artifacts filter_map returns
    // None.
    assert!(task.artifacts.is_none());
}

#[test]
fn build_multiturn_keeps_task_and_context_ids() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "h"),
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "f".to_string(),
        total_iterations: 1,
    });
    assert_eq!(task.id, tid);
    assert_eq!(task.context_id, ctx);
}

#[test]
fn build_multiturn_uses_system_agent_name_in_metadata() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "h"),
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "f".to_string(),
        total_iterations: 1,
    });
    let metadata = task.metadata.unwrap();
    assert!(!metadata.agent_name.is_empty());
}

#[test]
fn build_multiturn_history_tool_results_message_is_user_role() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: tid.clone(),
        user_message: user_message(&ctx, &tid, "x"),
        tool_calls: vec![call("a")],
        tool_results: vec![success_result("ok")],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    let history = task.history.unwrap();
    // The "tool_results: true" message is the third entry (index 2) and is User
    // role.
    let tool_results_msg = &history[2];
    assert_eq!(tool_results_msg.role, MessageRole::User);
    let metadata = tool_results_msg.metadata.as_ref().unwrap();
    assert_eq!(metadata.get("tool_results"), Some(&serde_json::json!(true)));
}

#[test]
fn build_multiturn_set_completed_state() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx,
        task_id: tid.clone(),
        user_message: Message {
            role: MessageRole::User,
            parts: vec![Part::Text(TextPart {
                text: "x".to_string(),
            })],
            message_id: MessageId::generate(),
            task_id: Some(tid),
            context_id: ContextId::generate(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        },
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "done".to_string(),
        total_iterations: 1,
    });
    assert_eq!(task.status.state, TaskState::Completed);
    assert!(task.created_at.is_some());
    assert!(task.last_modified.is_some());
}
