use anyhow::Result;
use systemprompt_models::a2a::TaskState;
use systemprompt_test_fixtures::agent_reads::TaskReadsExt;
use systemprompt_traits::RepositoryError;

use crate::common::Fixture;

#[tokio::test]
async fn completed_cannot_be_reopened_to_working() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    fx.repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .set_task_state(&task_id, TaskState::Completed, &now)
        .await?;

    let err = fx
        .repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await
        .expect_err("completed -> working must be rejected");
    assert!(
        matches!(err, RepositoryError::Conflict { .. }),
        "expected Conflict, got {err:?}"
    );

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_COMPLETED");

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn failed_cannot_be_reopened_to_working() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    fx.repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .set_task_state(&task_id, TaskState::Failed, &now)
        .await?;

    let err = fx
        .repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await
        .expect_err("failed -> working must be rejected");
    assert!(matches!(err, RepositoryError::Conflict { .. }));

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_FAILED");

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn canceled_cannot_transition_anywhere() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    fx.repo
        .set_task_state(&task_id, TaskState::Canceled, &now)
        .await?;

    for target in [
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::InputRequired,
    ] {
        let err = fx
            .repo
            .set_task_state(&task_id, target, &now)
            .await
            .expect_err(&format!("canceled -> {target:?} must be rejected"));
        assert!(matches!(err, RepositoryError::Conflict { .. }));
    }

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_CANCELED");

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn identity_transition_is_idempotent() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    fx.repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .set_task_state(&task_id, TaskState::Working, &now)
        .await?;

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_WORKING");

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn every_terminal_state_stamps_completion() -> Result<()> {
    let fx = Fixture::new().await?;
    let now = chrono::Utc::now();

    for terminal in [
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
    ] {
        let task_id = fx.insert_task(TaskState::Submitted).await?;
        fx.repo.set_task_state(&task_id, terminal, &now).await?;

        let (completed, timed): (bool, bool) = sqlx::query_as(
            "SELECT completed_at IS NOT NULL, execution_time_ms IS NOT NULL FROM agent_tasks \
             WHERE task_id = $1",
        )
        .bind(task_id.as_str())
        .fetch_one(&fx.pool)
        .await?;
        assert!(completed, "{terminal:?} stamps completed_at");
        assert!(timed, "{terminal:?} records execution_time_ms");
    }

    fx.cleanup().await?;
    Ok(())
}
