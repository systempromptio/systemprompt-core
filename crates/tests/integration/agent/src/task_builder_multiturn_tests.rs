//! Pure-function coverage for `build_multiturn_task` — exercises the
//! 176-line builder that assembles a Completed task whose `history` records
//! each tool-execution iteration. `task_builder_tests.rs` only hits the
//! simpler `build_completed_task` / `build_canceled_task` / `build_mock_task`
//! / `build_submitted_task` builders.

use rmcp::model::ContentBlock;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::processing::task_builder::{
    BuildMultiturnTaskParams, build_multiturn_task,
};
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{CallToolResult, ToolCall};

fn user_msg(ctx: &ContextId, task: &TaskId, text: &str) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart { text: text.into() })],
        message_id: MessageId::generate(),
        task_id: Some(task.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn tool_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call-{name}")),
        name: name.to_owned(),
        arguments: serde_json::json!({}),
    }
}

fn text_content(text: &str) -> ContentBlock {
    ContentBlock::text(text.to_owned())
}

fn success_result(text: &str) -> CallToolResult {
    CallToolResult::success(vec![text_content(text)])
}

#[test]
fn multiturn_with_no_tool_calls_has_only_user_and_final_messages() {
    let ctx = ContextId::generate();
    let task = TaskId::new("mt-empty");
    let user = user_msg(&ctx, &task, "hi");

    let result = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx.clone(),
        task_id: task.clone(),
        user_message: user,
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "no tools needed".to_owned(),
        total_iterations: 0,
    });

    assert_eq!(result.id, task);
    assert_eq!(result.context_id, ctx);
    assert!(matches!(result.status.state, TaskState::Completed));
    let history = result.history.expect("history present");
    // user message + final synthesis message
    assert_eq!(history.len(), 2, "got {history:?}");
    assert!(matches!(history[0].role, MessageRole::User));
    assert!(matches!(history.last().unwrap().role, MessageRole::Agent));
}

#[test]
fn multiturn_with_one_tool_call_builds_three_extra_history_entries() {
    let ctx = ContextId::generate();
    let task = TaskId::new("mt-one");
    let user = user_msg(&ctx, &task, "search");

    let result = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx,
        task_id: task,
        user_message: user,
        tool_calls: vec![tool_call("search")],
        tool_results: vec![success_result("ok")],
        final_response: "done".to_owned(),
        total_iterations: 1,
    });

    let history = result.history.expect("history present");
    // user + (agent-tool-call + user-tool-result) + final-synthesis
    assert_eq!(history.len(), 4, "got {history:?}");
    let final_msg = result.status.message.expect("status message");
    assert!(matches!(final_msg.role, MessageRole::Agent));
}

#[test]
fn multiturn_carries_total_iterations_and_tool_count_in_metadata() {
    let ctx = ContextId::generate();
    let task = TaskId::new("mt-meta");
    let user = user_msg(&ctx, &task, "x");
    let calls = vec![tool_call("a"), tool_call("b")];
    let results = vec![success_result("ra"), success_result("rb")];

    let result = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx,
        task_id: task,
        user_message: user,
        tool_calls: calls,
        tool_results: results,
        final_response: "synthesised".to_owned(),
        total_iterations: 3,
    });

    let metadata = result.metadata.expect("metadata present");
    let ext = metadata.extensions;
    assert_eq!(
        ext.get("total_iterations").and_then(|v| v.as_u64()),
        Some(3)
    );
    assert_eq!(
        ext.get("total_tools_called").and_then(|v| v.as_u64()),
        Some(2)
    );
}

#[test]
fn multiturn_without_structured_content_yields_no_artifacts() {
    // Tool results without `structured_content` cannot be parsed into a
    // ToolResponse, so `build_artifacts` filters them and the task ends with
    // `artifacts = None`.
    let ctx = ContextId::generate();
    let task = TaskId::new("mt-no-art");
    let user = user_msg(&ctx, &task, "x");

    let result = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx,
        task_id: task,
        user_message: user,
        tool_calls: vec![tool_call("t1")],
        tool_results: vec![success_result("plain text only")],
        final_response: "done".to_owned(),
        total_iterations: 1,
    });

    assert!(
        result.artifacts.is_none(),
        "expected no artifacts, got {:?}",
        result.artifacts
    );
}

#[test]
fn multiturn_final_status_message_carries_response_text() {
    let ctx = ContextId::generate();
    let task = TaskId::new("mt-final");
    let user = user_msg(&ctx, &task, "x");

    let result = build_multiturn_task(BuildMultiturnTaskParams {
        context_id: ctx,
        task_id: task,
        user_message: user,
        tool_calls: vec![],
        tool_results: vec![],
        final_response: "the final answer".to_owned(),
        total_iterations: 0,
    });

    let final_msg = result.status.message.expect("status message");
    let text_part = final_msg
        .parts
        .iter()
        .find_map(|p| match p {
            Part::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .expect("text part");
    assert_eq!(text_part, "the final answer");
}
