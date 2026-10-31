use rmcp::ErrorData as McpError;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::{ClientProfile, McpToolExecutor};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::brain::mcp::{BrainMcpTools, BrainToolCall, TOOL_NAMES};

use crate::brain_consumer_start::handle_for;
use crate::documents_mcp_rig::rig;
use crate::documents_mcp_support::{ctx_for, registered};
use crate::mcp_dispatch::{executor, refused, request, succeeded};

async fn call(
    tools: &BrainMcpTools,
    executor: &McpToolExecutor,
    ctx: &RequestContext,
    name: &str,
    args: Value,
) -> Result<CallToolResult, McpError> {
    let request = request(name, args);
    tools
        .call(BrainToolCall {
            executor,
            request: &request,
            context: ctx,
            client: &ClientProfile::unknown(),
        })
        .await
}

fn code(outcome: &Result<CallToolResult, McpError>) -> Option<String> {
    outcome
        .as_ref()
        .ok()
        .and_then(|result| result.structured_content.as_ref())
        .and_then(|content| content["code"].as_str())
        .map(str::to_owned)
}

#[tokio::test]
async fn every_brain_tool_dispatches_and_governing_tools_are_forbidden() {
    let r = rig(false).await;
    let tools = handle_for(&r).mcp_tools().expect("tools");
    let exec = executor("brain").await;
    let user = registered("brain-dispatch").await;
    let ctx = ctx_for(Some(&user));
    let server = McpServerId::new("brain");

    let listed: Vec<String> = tools
        .list_tools(&server)
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(listed, TOOL_NAMES.to_vec());

    let guide = call(&tools, &exec, &ctx, "brain_guide", json!({})).await;
    assert!(succeeded(&guide), "{guide:?}");
    let search = call(
        &tools,
        &exec,
        &ctx,
        "brain_search",
        json!({ "query": "nothing" }),
    )
    .await;
    assert!(succeeded(&search), "{search:?}");

    for name in [
        "brain_search",
        "brain_propose",
        "brain_dispute",
        "brain_why",
        "brain_recall_preview",
    ] {
        let outcome = call(&tools, &exec, &ctx, name, json!({ "bogus": 1 })).await;
        assert!(refused(&outcome), "{name} accepted bad input: {outcome:?}");
    }

    for name in ["brain_confirm", "brain_erase", "brain_resolve"] {
        let outcome = call(&tools, &exec, &ctx, name, json!({})).await;
        assert!(refused(&outcome), "{name}: {outcome:?}");
        assert_eq!(code(&outcome).as_deref(), Some("agent_forbidden"), "{name}");
    }

    let unknown = call(&tools, &exec, &ctx, "brain_nope", json!({}))
        .await
        .expect_err("unknown tool");
    assert!(unknown.message.contains("Unknown tool: 'brain_nope'"));
    assert!(unknown.message.contains("brain_search"));
}
