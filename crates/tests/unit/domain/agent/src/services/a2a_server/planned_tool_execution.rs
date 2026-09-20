// DB-backed tests for the planned strategy's tool-call branch: a successful
// tool run synthesizes and emits the final response, a plan whose template
// references an unplanned tool is funneled through the validation-failure
// explanation, a failing tool with working synthesis still yields a response,
// and a failing tool plus failing synthesis surfaces the tool errors.

use std::sync::Arc;

use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::a2a_server::processing::message::StreamEvent;
use systemprompt_agent::services::a2a_server::processing::strategies::{
    ExecutionContext, ExecutionStrategy, PlannedAgenticStrategy,
};
use systemprompt_identifiers::AgentName;
use systemprompt_models::ai::{PlannedToolCall, PlanningResult};
use tokio::sync::mpsc;

use super::a2a_helpers::{StubAiProvider, request_context, runtime_info};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

const AGENT: &str = "planned_exec_agent";

struct Harness {
    context: ExecutionContext,
    rx: mpsc::Receiver<StreamEvent>,
}

async fn harness_or_skip(provider: StubAiProvider) -> Option<Harness> {
    let pool = try_pool_or_skip().await?;
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos_handle, &user, &session).await;

    let (tx, rx) = mpsc::channel(64);
    let request_ctx = request_context(&ctx, &session, &user, AGENT);
    let context = ExecutionContext {
        ai_service: Arc::new(provider),
        skill_service: Arc::new(super::a2a_helpers::skill_service(&pool)),
        agent_runtime: runtime_info(AGENT),
        agent_name: AgentName::try_new(AGENT).expect("valid AgentName"),
        task_id,
        context_id: ctx,
        tx,
        request_ctx,
        execution_step_repo: Arc::new(ExecutionStepRepository::new(&pool).expect("exec repo")),
    };
    Some(Harness { context, rx })
}

fn success_result(payload: serde_json::Value) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text("ok".to_owned())]);
    result.structured_content = Some(payload);
    result
}

fn error_result(message: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.to_owned())])
}

fn drain(rx: &mut mpsc::Receiver<StreamEvent>) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

#[tokio::test]
async fn successful_tool_run_synthesizes_and_emits_response() {
    let provider = StubAiProvider::new()
        .with_plan(PlanningResult::tool_calls(
            "look it up",
            vec![PlannedToolCall::new("alpha", json!({"q": 1}))],
        ))
        .with_tool_result("alpha", success_result(json!({"answer": 42})))
        .with_response("final answer");
    let Some(Harness { context, mut rx }) = harness_or_skip(provider).await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;

    let result = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect("planned execution succeeds");

    assert_eq!(result.accumulated_text, "final answer");
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name, "alpha");
    assert_eq!(result.tool_results.len(), 1);
    assert_eq!(result.iterations, 1);

    let events = drain(&mut rx);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Text(t) if t == "final answer")),
        "final response must be streamed"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ExecutionStepUpdate { .. })),
        "execution steps must be streamed"
    );
}

#[tokio::test]
async fn direct_response_plan_streams_text_without_tool_calls() {
    let provider =
        StubAiProvider::new().with_plan(PlanningResult::direct_response("no tools required"));
    let Some(Harness { context, mut rx }) = harness_or_skip(provider).await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;

    let result = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect("direct response succeeds");

    assert_eq!(result.accumulated_text, "no tools required");
    assert!(result.tool_calls.is_empty());
    assert!(result.tool_results.is_empty());
    assert_eq!(result.iterations, 1);

    let events = drain(&mut rx);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Text(t) if t == "no tools required")),
        "direct response must be streamed"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ExecutionStepUpdate { .. })),
        "planning steps must be streamed"
    );
}

#[tokio::test]
async fn template_referencing_unplanned_tool_returns_validation_explanation() {
    let provider = StubAiProvider::new()
        .with_plan(PlanningResult::tool_calls(
            "bad plan",
            vec![PlannedToolCall::new(
                "alpha",
                json!({"x": "$missing.output.value"}),
            )],
        ))
        .with_response("that plan is invalid");
    let Some(Harness { context, mut rx }) = harness_or_skip(provider).await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;

    let result = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect("validation failure resolves to an explanation");

    assert_eq!(result.accumulated_text, "that plan is invalid");
    assert!(result.tool_calls.is_empty());
    assert!(result.tool_results.is_empty());

    let events = drain(&mut rx);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Text(t) if t == "that plan is invalid")),
        "explanation must be streamed"
    );
}

#[tokio::test]
async fn failing_tool_with_working_synthesis_still_responds() {
    let provider = StubAiProvider::new()
        .with_plan(PlanningResult::tool_calls(
            "try anyway",
            vec![PlannedToolCall::new("broken", json!({}))],
        ))
        .with_tool_result("broken", error_result("boom"))
        .with_response("recovered gracefully");
    let Some(Harness { context, rx }) = harness_or_skip(provider).await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;

    let result = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect("synthesis covers the tool failure");

    assert_eq!(result.accumulated_text, "recovered gracefully");
    assert_eq!(result.tool_results.len(), 1);
    drop(rx);
}

#[tokio::test]
async fn failing_tool_and_failing_synthesis_surface_tool_errors() {
    let provider = StubAiProvider::new()
        .with_plan(PlanningResult::tool_calls(
            "doomed",
            vec![PlannedToolCall::new("broken", json!({}))],
        ))
        .with_tool_result("broken", error_result("boom"))
        .with_failing_response();
    let Some(Harness { context, rx: _rx }) = harness_or_skip(provider).await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;

    let error = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect_err("tool errors must surface when synthesis also fails");

    assert!(
        error.to_string().contains("Tool execution failed"),
        "got {error}"
    );
}

#[tokio::test]
async fn multiple_successful_tools_persist_one_completed_batch_summary_and_stream_result() {
    use systemprompt_models::{StepContent, StepStatus};

    let provider = StubAiProvider::new()
        .with_plan(PlanningResult::tool_calls(
            "combine both sources",
            vec![
                PlannedToolCall::new("alpha", json!({"q": 1})),
                PlannedToolCall::new("beta", json!({"q": 2})),
            ],
        ))
        .with_tool_result("alpha", success_result(json!({"answer": "a"})))
        .with_tool_result("beta", success_result(json!({"answer": "b"})))
        .with_response("combined answer");
    let Harness { context, mut rx } = harness_or_skip(provider)
        .await
        .expect("planned strategy database fixture");
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let task_id = context.task_id.clone();
    let steps = Arc::clone(&context.execution_step_repo);

    let result = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect("multi-tool execution succeeds");

    assert_eq!(result.accumulated_text, "combined answer");
    assert_eq!(
        result
            .tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "beta"]
    );
    assert_eq!(result.tool_results.len(), 2);
    assert!(
        result
            .tool_results
            .iter()
            .all(|result| result.is_error == Some(false))
    );

    let persisted = steps.list_by_task(&task_id).await.expect("persisted steps");
    let tool_steps = persisted
        .iter()
        .filter(|step| matches!(step.content, StepContent::ToolExecution { .. }))
        .collect::<Vec<_>>();
    assert_eq!(tool_steps.len(), 1, "one batch produces one tool step");
    let tool_step = tool_steps[0];
    assert_eq!(tool_step.status, StepStatus::Completed);
    let StepContent::ToolExecution {
        tool_name,
        tool_arguments,
        tool_result,
    } = &tool_step.content
    else {
        unreachable!()
    };
    assert_eq!(tool_name, "2 tools");
    assert_eq!(
        tool_arguments,
        &json!([
            {"tool": "alpha", "arguments": {"q": 1}},
            {"tool": "beta", "arguments": {"q": 2}}
        ])
    );
    let recorded = tool_result
        .as_ref()
        .and_then(|value| value["results"].as_array())
        .expect("multi-tool result summary");
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0]["tool"], "alpha");
    assert_eq!(recorded[0]["output"], json!({"answer": "a"}));
    assert_eq!(recorded[1]["tool"], "beta");
    assert_eq!(recorded[1]["output"], json!({"answer": "b"}));

    let events = drain(&mut rx);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::Text(text) if text == "combined answer")),
        "final synthesis is emitted: {events:?}"
    );
}

#[tokio::test]
async fn successful_then_failed_tool_persists_failed_batch_with_exact_diagnosis() {
    use systemprompt_models::{StepContent, StepStatus};

    let provider = StubAiProvider::new()
        .with_plan(PlanningResult::tool_calls(
            "try the dependent operations",
            vec![
                PlannedToolCall::new("alpha", json!({"q": 1})),
                PlannedToolCall::new("broken", json!({"q": 2})),
                PlannedToolCall::new("never-run", json!({})),
            ],
        ))
        .with_tool_result("alpha", success_result(json!({"answer": 42})))
        .with_tool_result("broken", error_result("upstream unavailable"))
        .with_tool_result("never-run", success_result(json!({"unexpected": true})))
        .with_response("partial answer with failure guidance");
    let Harness { context, mut rx } = harness_or_skip(provider)
        .await
        .expect("planned strategy database fixture");
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let task_id = context.task_id.clone();
    let steps = Arc::clone(&context.execution_step_repo);

    let result = PlannedAgenticStrategy::new()
        .execute(context, Vec::new())
        .await
        .expect("tool failure is synthesized for the caller");

    assert_eq!(
        result.accumulated_text,
        "partial answer with failure guidance"
    );
    assert_eq!(
        result
            .tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "broken", "never-run"],
        "the wire result retains the declared plan"
    );
    assert_eq!(
        result.tool_results.len(),
        2,
        "execution halts after failure"
    );
    assert_eq!(result.tool_results[0].is_error, Some(false));
    assert_eq!(result.tool_results[1].is_error, Some(true));

    let persisted = steps.list_by_task(&task_id).await.expect("persisted steps");
    let tool_steps = persisted
        .iter()
        .filter(|step| matches!(step.content, StepContent::ToolExecution { .. }))
        .collect::<Vec<_>>();
    assert_eq!(tool_steps.len(), 1, "one batch produces one tool step");
    let tool_step = tool_steps[0];
    assert_eq!(tool_step.status, StepStatus::Failed);
    assert_eq!(
        tool_step.error_message.as_deref(),
        Some("internal error: Tool broken failed: upstream unavailable")
    );
    assert!(
        matches!(
            &tool_step.content,
            StepContent::ToolExecution { tool_name, tool_arguments, tool_result }
                if tool_name == "3 tools"
                    && tool_arguments.as_array().is_some_and(|items| items.len() == 3)
                    && tool_result.is_none()
        ),
        "failed batch records its declared tools without fabricating a success result: {tool_step:?}"
    );

    let events = drain(&mut rx);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::Text(text) if text == "partial answer with failure guidance")),
        "failure-aware synthesis is emitted: {events:?}"
    );
}
