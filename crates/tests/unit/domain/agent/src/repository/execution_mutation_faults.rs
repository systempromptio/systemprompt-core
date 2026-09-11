//! What the execution-step write paths report when the database is unreachable.
//!
//! Every mutation wraps its sqlx failure in a message naming both the operation
//! and the step or task it was working on. That is the whole diagnostic an
//! operator gets from an agent run that died mid-task, so a raw driver error
//! reaching the caller unlabelled is a regression.

use chrono::Utc;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_identifiers::TaskId;
use systemprompt_models::{ExecutionStep, StepId};
use systemprompt_test_fixtures::closed_db_pool;

async fn repository() -> ExecutionStepRepository {
    let pool = closed_db_pool().await;
    ExecutionStepRepository::new(&pool).expect("a lazy pool still yields a repository")
}

#[tokio::test]
async fn creating_a_step_against_a_dead_pool_fails_rather_than_reporting_success() {
    let repo = repository().await;
    let step = ExecutionStep::tool_execution(
        TaskId::new("task-create-fault"),
        "search",
        serde_json::json!({"q": "rust"}),
    );

    let error = repo
        .create(&step)
        .await
        .expect_err("an unwritten step must never be reported as created");

    assert!(
        error
            .to_string()
            .contains("Failed to create execution step"),
        "the failure must say which write failed: {error}"
    );
}

#[tokio::test]
async fn completing_a_step_with_a_tool_result_names_the_step_it_could_not_write() {
    let repo = repository().await;
    let step_id = StepId::new();

    let error = repo
        .complete_step(&step_id, Utc::now(), Some(serde_json::json!({"hits": 3})))
        .await
        .expect_err("a completion that never landed must surface as an error");

    let message = error.to_string();
    assert!(
        message.contains("complete execution step"),
        "the operation must be named: {message}"
    );
    assert!(
        message.contains(step_id.as_str()),
        "the step id is the only handle an operator has: {message}"
    );
}

#[tokio::test]
async fn completing_a_step_without_a_tool_result_reports_the_same_failure() {
    let repo = repository().await;
    let step_id = StepId::new();

    let message = repo
        .complete_step(&step_id, Utc::now(), None)
        .await
        .expect_err("the no-result branch is not exempt from reporting failure")
        .to_string();

    assert!(
        message.contains("complete execution step") && message.contains(step_id.as_str()),
        "both write branches must report identically: {message}"
    );
}

#[tokio::test]
async fn failing_a_step_that_cannot_be_recorded_names_the_step() {
    let repo = repository().await;
    let step_id = StepId::new();

    let message = repo
        .fail_step(&step_id, Utc::now(), "tool timed out")
        .await
        .expect_err("recording a failure can itself fail, and must say so")
        .to_string();

    assert!(
        message.contains("fail execution step") && message.contains(step_id.as_str()),
        "a lost failure record must not pass silently: {message}"
    );
}

#[tokio::test]
async fn sweeping_in_progress_steps_for_a_task_reports_the_task_it_could_not_reach() {
    let repo = repository().await;
    let task_id = TaskId::new("task-sweep-fault");

    let message = repo
        .fail_in_progress_steps_for_task(&task_id, "worker died")
        .await
        .expect_err("an unreachable sweep must not report zero rows as success")
        .to_string();

    assert!(
        message.contains("fail in-progress steps for task") && message.contains(task_id.as_str()),
        "the orphan sweep must name the task: {message}"
    );
}

#[tokio::test]
async fn completing_a_planning_step_reports_the_step_rather_than_a_bare_driver_error() {
    let repo = repository().await;
    let step_id = StepId::new();

    let message = repo
        .complete_planning_step(&step_id, Utc::now(), Some("use search".to_owned()), None)
        .await
        .expect_err("a planning step that never landed has no row to return")
        .to_string();

    assert!(
        message.contains("complete planning step") && message.contains(step_id.as_str()),
        "the planning write must be distinguishable from the tool write: {message}"
    );
}
