// `ContextToolExecutor` — the `ToolExecutorTrait` implementation the planned
// strategy hands to the plan executor. Nothing calls it directly, so none of
// its four outcomes (structured result, no result, tool-reported error,
// missing structured_content) has ever run.

use std::sync::Arc;

use rmcp::model::{CallToolResult, ContentBlock};
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::SkillService;
use systemprompt_agent::services::a2a_server::processing::message::StreamEvent;
use systemprompt_agent::services::a2a_server::processing::strategies::{
    ContextToolExecutor, ExecutionContext, ToolExecutorTrait,
};
use systemprompt_identifiers::AgentName;
use tokio::sync::mpsc;

use super::a2a_helpers::{StubAiProvider, request_context, runtime_info};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

const AGENT: &str = "ctx_tool_exec_agent";

async fn executor_or_skip(provider: StubAiProvider) -> Option<ContextToolExecutor> {
    let pool = try_pool_or_skip().await?;
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos_handle, &user, &session).await;

    let (tx, rx) = mpsc::channel::<StreamEvent>(64);
    // The receiver must outlive the executor or every send fails; leak it so the
    // channel stays open for the duration of the call under test.
    std::mem::forget(rx);
    let request_ctx = request_context(&ctx, &session, &user, AGENT);

    Some(ContextToolExecutor {
        context: ExecutionContext {
            ai_service: Arc::new(provider),
            skill_service: Arc::new(SkillService::new().expect("skill service")),
            agent_runtime: runtime_info(AGENT),
            agent_name: AgentName::new(AGENT),
            task_id,
            context_id: ctx,
            tx,
            request_ctx,
            execution_step_repo: Arc::new(ExecutionStepRepository::new(&pool).expect("exec repo")),
        },
    })
}

fn success_with_structured(payload: serde_json::Value) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text("ok".to_owned())]);
    result.structured_content = Some(payload);
    result
}

#[tokio::test]
async fn a_successful_tool_call_returns_its_structured_content() {
    let provider = StubAiProvider::new().with_tool_result(
        "lookup",
        success_with_structured(serde_json::json!({"rows": 3})),
    );
    let Some(executor) = executor_or_skip(provider).await else {
        return;
    };
    let ctx = executor.context.request_ctx.clone();

    let value = executor
        .execute_tool("lookup", serde_json::json!({"q": "x"}), &[], &ctx)
        .await
        .expect("the tool succeeded");

    assert_eq!(
        value.output,
        serde_json::json!({"rows": 3}),
        "the caller receives the structured payload, not the text block"
    );
}

#[tokio::test]
async fn a_tool_that_returns_no_result_is_an_error_naming_the_tool() {
    let Some(executor) = executor_or_skip(StubAiProvider::new()).await else {
        return;
    };
    let ctx = executor.context.request_ctx.clone();

    let err = executor
        .execute_tool("absent", serde_json::json!({}), &[], &ctx)
        .await
        .expect_err("no result came back");

    assert!(
        err.to_string().contains("absent") && err.to_string().contains("no result"),
        "the failure identifies which tool produced nothing: {err}"
    );
}

#[tokio::test]
async fn a_tool_reporting_an_error_surfaces_its_message() {
    let mut failing = CallToolResult::error(vec![ContentBlock::text(
        "upstream rejected the query".to_owned(),
    )]);
    failing.is_error = Some(true);

    let Some(executor) =
        executor_or_skip(StubAiProvider::new().with_tool_result("broken", failing)).await
    else {
        return;
    };
    let ctx = executor.context.request_ctx.clone();

    let err = executor
        .execute_tool("broken", serde_json::json!({}), &[], &ctx)
        .await
        .expect_err("the tool reported failure");

    let message = err.to_string();
    assert!(message.contains("broken"), "got: {message}");
    assert!(
        message.contains("upstream rejected the query"),
        "the tool's own error text reaches the caller: {message}"
    );
}

#[tokio::test]
async fn a_tool_error_without_text_content_falls_back_to_unknown() {
    let mut failing = CallToolResult::error(vec![]);
    failing.is_error = Some(true);

    let Some(executor) =
        executor_or_skip(StubAiProvider::new().with_tool_result("silent", failing)).await
    else {
        return;
    };
    let ctx = executor.context.request_ctx.clone();

    let err = executor
        .execute_tool("silent", serde_json::json!({}), &[], &ctx)
        .await
        .expect_err("the tool reported failure");

    assert!(
        err.to_string().contains("Unknown error"),
        "an error with no message still produces a reportable failure: {err}"
    );
}

#[tokio::test]
async fn a_successful_tool_without_structured_content_is_rejected() {
    let bare = CallToolResult::success(vec![ContentBlock::text("prose only".to_owned())]);

    let Some(executor) =
        executor_or_skip(StubAiProvider::new().with_tool_result("prose", bare)).await
    else {
        return;
    };
    let ctx = executor.context.request_ctx.clone();

    let err = executor
        .execute_tool("prose", serde_json::json!({}), &[], &ctx)
        .await
        .expect_err("a planned step needs machine-readable output");

    assert!(
        err.to_string().contains("no structured_content"),
        "prose alone cannot feed the next planned step: {err}"
    );
}
