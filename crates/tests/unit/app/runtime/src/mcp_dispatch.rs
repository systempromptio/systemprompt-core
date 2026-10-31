use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::{Map, Value, json};
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::{ArtifactIngest, ClientProfile, McpToolExecutor};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{DocumentsMcpTools, DocumentsToolCall, TOOL_NAMES};
use systemprompt_runtime::issues::mcp::{IssuesMcpTools, IssuesToolCall};

use crate::documents_mcp_rig::rig;
use crate::documents_mcp_support::{ctx_for, registered};

pub(crate) async fn executor(server: &str) -> McpToolExecutor {
    let db = systemprompt_test_fixtures::test_db_pool().await;
    McpToolExecutor::new(
        Arc::new(ToolUsageRepository::new(&db)),
        Arc::new(AiRequestRepository::new(&db)),
        Arc::new(ArtifactIngest::from_db(&db, None)),
        McpServerId::new(server),
    )
}

pub(crate) fn request(name: &str, args: Value) -> CallToolRequestParams {
    let arguments: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    serde_json::from_value(json!({ "name": name, "arguments": arguments })).expect("request")
}

async fn documents(
    tools: &DocumentsMcpTools,
    executor: &McpToolExecutor,
    ctx: &RequestContext,
    name: &str,
    args: Value,
) -> Result<CallToolResult, McpError> {
    let request = request(name, args);
    tools
        .call(DocumentsToolCall {
            executor,
            request: &request,
            context: ctx,
            client: &ClientProfile::unknown(),
        })
        .await
}

async fn issues(
    tools: &IssuesMcpTools,
    executor: &McpToolExecutor,
    ctx: &RequestContext,
    name: &str,
    args: Value,
) -> Result<CallToolResult, McpError> {
    let request = request(name, args);
    tools
        .call(IssuesToolCall {
            executor,
            request: &request,
            context: ctx,
            client: &ClientProfile::unknown(),
        })
        .await
}

pub(crate) fn refused(outcome: &Result<CallToolResult, McpError>) -> bool {
    match outcome {
        Err(_) => true,
        Ok(result) => result.is_error == Some(true),
    }
}

pub(crate) fn succeeded(outcome: &Result<CallToolResult, McpError>) -> bool {
    matches!(outcome, Ok(result) if result.is_error != Some(true))
}

#[tokio::test]
async fn every_documents_tool_dispatches_and_unknown_names_are_refused() {
    let r = rig(true).await;
    let exec = executor("documents").await;
    let user = registered("dispatch-docs").await;
    let ctx = ctx_for(Some(&user));

    let listed: Vec<String> = r
        .tools
        .list_tools()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    for name in TOOL_NAMES {
        assert!(listed.iter().any(|tool| tool == name), "{name} not listed");
    }

    for name in ["documents_list"] {
        let outcome = documents(&r.tools, &exec, &ctx, name, json!({})).await;
        assert!(succeeded(&outcome), "{name}: {outcome:?}");
    }
    for name in TOOL_NAMES
        .iter()
        .filter(|name| !matches!(**name, "documents_list" | "documents_inbox"))
    {
        let outcome = documents(&r.tools, &exec, &ctx, name, json!({ "bogus": 1 })).await;
        assert!(refused(&outcome), "{name} accepted bad input: {outcome:?}");
    }

    let unknown = documents(&r.tools, &exec, &ctx, "documents_nope", json!({}))
        .await
        .expect_err("unknown tool");
    assert!(unknown.message.contains("Unknown tool: 'documents_nope'"));
    assert!(unknown.message.contains("documents_read"));
}

#[tokio::test]
async fn every_issue_tool_dispatches_and_unknown_names_are_refused() {
    let r = rig(true).await;
    let tools = IssuesMcpTools::new(
        Arc::clone(r.runtime.issues()),
        Some(Arc::clone(r.runtime.uploads())),
        systemprompt_test_fixtures::test_token_authority("issues-dispatch"),
        McpServerId::new("issues"),
        "https://issues.example.test/".to_owned(),
    );
    let exec = executor("issues").await;
    let user = registered("dispatch-issues").await;
    let ctx = ctx_for(Some(&user));

    let listed: Vec<String> = tools
        .list_tools()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(listed, IssuesMcpTools::tool_names().to_vec());
    assert_eq!(tools.server_name().as_str(), "issues");

    let listed = issues(&tools, &exec, &ctx, "issues_list", json!({})).await;
    assert!(succeeded(&listed), "{listed:?}");
    for name in ["report_issue", "issue_attachment", "issue_status"] {
        let outcome = issues(&tools, &exec, &ctx, name, json!({ "bogus": 1 })).await;
        assert!(refused(&outcome), "{name} accepted bad input: {outcome:?}");
    }

    let unknown = issues(&tools, &exec, &ctx, "issue_nope", json!({}))
        .await
        .expect_err("unknown tool");
    assert!(unknown.message.contains("Unknown tool: 'issue_nope'"));
    assert!(unknown.message.contains("report_issue"));
}
