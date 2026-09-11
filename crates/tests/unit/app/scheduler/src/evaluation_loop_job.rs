//! Context requirements of the scheduled auto-improve evaluation pass.
//!
//! The job reads its collaborators out of the type-erased `JobContext`. A
//! context assembled without them must fail loudly and name what is missing,
//! never run a judging pass against a half-built dependency graph.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_scheduler::jobs::EvaluationLoopJob;
use systemprompt_traits::{Job, JobContext};

fn context(
    db_pool: Arc<dyn std::any::Any + Send + Sync>,
    app_context: Arc<dyn std::any::Any + Send + Sync>,
) -> JobContext {
    JobContext::new(
        Actor::job(UserId::new("evaluation-loop-test"), "test".to_owned()),
        db_pool,
        app_context,
        Arc::new(()),
    )
}

#[tokio::test]
async fn a_context_without_a_database_pool_fails_and_names_it() {
    let error = EvaluationLoopJob
        .execute(&context(Arc::new(()), Arc::new(())))
        .await
        .expect_err("the job must not run without a database pool");

    assert!(
        error.to_string().contains("DbPool"),
        "the failure must name the missing collaborator, got: {error}"
    );
}

#[tokio::test]
async fn the_job_is_scheduled_daily_and_describes_its_parameters() {
    let job = EvaluationLoopJob;

    assert_eq!(job.name(), "evaluation_loop");
    assert_eq!(
        job.schedule().split_whitespace().count(),
        6,
        "the schedule must be a six-field cron expression, got {:?}",
        job.schedule()
    );
    for parameter in [
        "sample_size",
        "window_hours",
        "rubric",
        "judge_provider",
        "judge_model",
        "budget_microdollars",
    ] {
        assert!(
            job.description().contains(parameter),
            "operators configure {parameter}, so the description must list it"
        );
    }
}

#[tokio::test]
async fn a_context_with_a_pool_but_no_app_context_fails_and_names_it() {
    let pool: systemprompt_database::DbPool = systemprompt_test_fixtures::closed_db_pool().await;

    let error = EvaluationLoopJob
        .execute(&context(Arc::new(pool), Arc::new(())))
        .await
        .expect_err("the job must not run without the application context");

    let message = error.to_string();
    assert!(
        message.contains("AppContext"),
        "the failure must name the missing collaborator, got: {message}"
    );
    assert!(
        !message.contains("DbPool"),
        "the pool was supplied, so it must not be blamed: {message}"
    );
}
