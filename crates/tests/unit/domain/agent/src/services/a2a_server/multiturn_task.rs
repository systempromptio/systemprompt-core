//! Unit tests for build_multiturn_task and its history/artifact builders.
//!
//! Target: crates/domain/agent/src/services/a2a_server/processing/task_builder/builders.rs

use rmcp::model::{CallToolResult, Content};
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
    let mut r = CallToolResult::success(vec![Content::text(text.to_string())]);
    r.structured_content = Some(serde_json::json!({"out": text}));
    r
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
    // user + (executing + results) + final = 4 (single iteration consumed both calls).
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
    let ext = metadata.extensions.unwrap();
    assert_eq!(ext.get("total_iterations"), Some(&serde_json::json!(7)));
    assert_eq!(ext.get("total_tools_called"), Some(&serde_json::json!(1)));
}

#[test]
fn build_multiturn_no_artifacts_when_no_structured_content() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let mut result_no_struct = CallToolResult::success(vec![Content::text("plain")]);
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
    // structured_content::is_error is None, so build_artifacts filter_map returns None.
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
    // The "tool_results: true" message is the third entry (index 2) and is User role.
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
