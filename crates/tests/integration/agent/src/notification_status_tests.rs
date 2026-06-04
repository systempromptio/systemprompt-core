use anyhow::Result;
use systemprompt_models::a2a::TaskState;
use systemprompt_traits::RepositoryError;

use crate::common::Fixture;

#[tokio::test]
async fn notification_cannot_reopen_completed_task() -> Result<()> {
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
        .apply_notification_status(&task_id, "working", &now)
        .await
        .expect_err("notification 'working' on completed task must be rejected");
    assert!(matches!(err, RepositoryError::ConstraintViolation(_)));

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_COMPLETED");

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn notification_rejects_unknown_state_string() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    let err = fx
        .repo
        .apply_notification_status(&task_id, "totally-bogus", &now)
        .await
        .expect_err("unknown notification state must be rejected");
    assert!(matches!(err, RepositoryError::InvalidData(_)));

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_SUBMITTED");

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn notification_accepts_short_and_long_state_aliases() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let now = chrono::Utc::now();

    fx.repo
        .apply_notification_status(&task_id, "working", &now)
        .await?;
    assert_eq!(fx.current_status(&task_id).await?, "TASK_STATE_WORKING");

    fx.repo
        .apply_notification_status(&task_id, "TASK_STATE_COMPLETED", &now)
        .await?;
    assert_eq!(fx.current_status(&task_id).await?, "TASK_STATE_COMPLETED");

    fx.cleanup().await?;
    Ok(())
}
