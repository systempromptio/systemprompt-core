//! What the execution-step write paths report when the database is unreachable.
//!
//! Every mutation surfaces the driver failure as a typed
//! `RepositoryError::Database` carrying the pool error as its source. A write
//! that never landed must never be reported as success, on either branch of a
//! write path.

use chrono::Utc;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_identifiers::TaskId;
use systemprompt_models::{ExecutionStep, StepId};
use systemprompt_test_fixtures::closed_db_pool;
use systemprompt_traits::RepositoryError;

async fn repository() -> ExecutionStepRepository {
    let pool = closed_db_pool().await;
    ExecutionStepRepository::new(&pool)
}

fn assert_database_failure(error: &RepositoryError) {
    assert!(
        matches!(error, RepositoryError::Database { sqlstate: None, .. }),
        "an unreachable pool is a database failure with its driver cause: {error:?}"
    );
    assert!(
        std::error::Error::source(error).is_some(),
        "the driver error is kept as the source: {error:?}"
    );
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

    assert_database_failure(&error);
}

#[tokio::test]
async fn completing_a_step_with_a_tool_result_surfaces_the_failed_write() {
    let repo = repository().await;
    let step_id = StepId::new();

    let error = repo
        .complete_step(&step_id, Utc::now(), Some(serde_json::json!({"hits": 3})))
        .await
        .expect_err("a completion that never landed must surface as an error");

    assert_database_failure(&error);
}

#[tokio::test]
async fn completing_a_step_without_a_tool_result_reports_the_same_failure() {
    let repo = repository().await;
    let step_id = StepId::new();

    let error = repo
        .complete_step(&step_id, Utc::now(), None)
        .await
        .expect_err("the no-result branch is not exempt from reporting failure");

    assert_database_failure(&error);
}

#[tokio::test]
async fn failing_a_step_that_cannot_be_recorded_is_an_error() {
    let repo = repository().await;
    let step_id = StepId::new();

    let error = repo
        .fail_step(&step_id, Utc::now(), "tool timed out")
        .await
        .expect_err("recording a failure can itself fail, and must say so");

    assert_database_failure(&error);
}

#[tokio::test]
async fn sweeping_in_progress_steps_for_an_unreachable_task_is_an_error() {
    let repo = repository().await;
    let task_id = TaskId::new("task-sweep-fault");

    let error = repo
        .fail_in_progress_steps_for_task(&task_id, "worker died")
        .await
        .expect_err("an unreachable sweep must not report zero rows as success");

    assert_database_failure(&error);
}

#[tokio::test]
async fn completing_a_planning_step_against_a_dead_pool_is_an_error() {
    let repo = repository().await;
    let step_id = StepId::new();

    let error = repo
        .complete_planning_step(&step_id, Utc::now(), Some("use search".to_owned()), None)
        .await
        .expect_err("a planning step that never landed has no row to return");

    assert_database_failure(&error);
}
