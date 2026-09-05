//! The stream processor's artifact gate.
//!
//! `build_artifacts_from_results` decides whether a round of tool calls
//! produces A2A artifacts at all. It runs on every streaming turn, and the
//! decision is made on `structured_content` alone: ephemeral tool calls that
//! returned only text must not manufacture artifacts, because an artifact is a
//! durable, user-visible record of a result.

use rmcp::model::{CallToolResult, ContentBlock};
use systemprompt_agent::test_api::build_artifacts_from_results;
use systemprompt_identifiers::{AiToolCallId, ContextId, McpServerId, TaskId};
use systemprompt_models::{McpTool, ToolCall};

fn call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("c-{name}")),
        name: name.to_owned(),
        arguments: serde_json::json!({}),
    }
}

fn tool(name: &str) -> McpTool {
    McpTool::new(name, McpServerId::new("svc"))
}

fn text_only_result() -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text("plain text".to_owned())])
}

// The builder reads the execution identifiers out of `_meta`; structured
// content without them is not a transformable artifact, so a fixture that
// omits them tests the transformer's rejection rather than this gate.
fn structured_result() -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text("text".to_owned())]);
    result.structured_content = Some(serde_json::json!({
        "rows": [],
        "_meta": {
            "io.systemprompt/execution": {
                "artifact_id": "art-stream-1",
                "mcp_execution_id": "exec-stream-1"
            }
        }
    }));
    result
}

fn build(results: &[CallToolResult], calls: &[ToolCall], tools: &[McpTool]) -> usize {
    build_artifacts_from_results(
        results,
        calls,
        tools,
        &ContextId::generate(),
        &TaskId::generate(),
    )
    .expect("artifact building must not fail on well-formed input")
    .len()
}

#[test]
fn a_turn_with_no_tool_results_produces_no_artifacts() {
    assert_eq!(build(&[], &[], &[]), 0);
}

// Why: this is the gate. A tool that answered in plain text produced nothing
// durable, and minting an artifact for it would put an empty record in the
// user's task history for every ephemeral call in the conversation.
#[test]
fn tool_results_carrying_only_text_produce_no_artifacts() {
    let count = build(
        &[text_only_result(), text_only_result()],
        &[call("echo"), call("ping")],
        &[tool("echo"), tool("ping")],
    );

    assert_eq!(
        count, 0,
        "results without structured_content are ephemeral and must not become artifacts"
    );
}

// Why: the converse. The gate is any-not-all, so one structured result in a
// batch must still open the path — dropping it would silently lose the only
// durable output of the turn.
#[test]
fn a_single_structured_result_among_text_ones_opens_the_artifact_path() {
    let count = build(
        &[text_only_result(), structured_result()],
        &[call("echo"), call("query")],
        &[tool("echo"), tool("query")],
    );

    assert!(
        count > 0,
        "a batch containing structured_content must reach the builder"
    );
}
