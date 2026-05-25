use anyhow::Result;
use systemprompt_models::a2a::TaskState;
use systemprompt_traits::RepositoryError;

use crate::common::Fixture;

#[tokio::test]
async fn completed_cannot_be_reopened_to_working() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    fx.repo
        .update_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .update_task_state(&task_id, TaskState::Completed, &now)
        .await?;

    let err = fx
        .repo
        .update_task_state(&task_id, TaskState::Working, &now)
        .await
        .expect_err("completed -> working must be rejected");
    assert!(
        matches!(err, RepositoryError::ConstraintViolation(_)),
        "expected ConstraintViolation, got {err:?}"
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
        .update_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .update_task_state(&task_id, TaskState::Failed, &now)
        .await?;

    let err = fx
        .repo
        .update_task_state(&task_id, TaskState::Working, &now)
        .await
        .expect_err("failed -> working must be rejected");
    assert!(matches!(err, RepositoryError::ConstraintViolation(_)));

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
        .update_task_state(&task_id, TaskState::Canceled, &now)
        .await?;

    for target in [
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::InputRequired,
    ] {
        let err = fx
            .repo
            .update_task_state(&task_id, target, &now)
            .await
            .expect_err(&format!("canceled -> {target:?} must be rejected"));
        assert!(matches!(err, RepositoryError::ConstraintViolation(_)));
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
        .update_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .update_task_state(&task_id, TaskState::Working, &now)
        .await?;
    fx.repo
        .update_task_state(&task_id, TaskState::Working, &now)
        .await?;

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_WORKING");

    fx.cleanup().await?;
    Ok(())
}
