use anyhow::Result;
use systemprompt_models::a2a::TaskState;

use crate::common::Fixture;

async fn insert_message(
    fx: &Fixture,
    task_id: &str,
    message_id: &str,
    role: &str,
    seq: i32,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "INSERT INTO task_messages (task_id, message_id, role, context_id, user_id, session_id, \
         trace_id, sequence_number) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(task_id)
    .bind(message_id)
    .bind(role)
    .bind(fx.context_id.as_str())
    .bind(fx.user_id.as_str())
    .bind(fx.session_id.as_str())
    .bind(fx.trace_id.as_str())
    .bind(seq)
    .execute(&fx.pool)
    .await
    .map(|r| r.rows_affected())
}

#[tokio::test]
async fn duplicate_message_id_is_rejected_by_unique_constraint() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;

    let msg = format!("msg_{}", fx.tag);
    insert_message(&fx, task_id.as_str(), &msg, "user", 1).await?;

    let err = insert_message(&fx, task_id.as_str(), &msg, "user", 2)
        .await
        .expect_err("duplicate message_id must be rejected");
    let s = format!("{err}");
    assert!(
        s.to_lowercase().contains("unique") || s.to_lowercase().contains("duplicate"),
        "expected uniqueness error, got {s}"
    );

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM task_messages WHERE task_id = $1 AND message_id = $2")
            .bind(task_id.as_str())
            .bind(&msg)
            .fetch_one(&fx.pool)
            .await?;
    assert_eq!(count.0, 1);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn same_message_id_allowed_across_distinct_tasks() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_a = fx.insert_task(TaskState::Submitted).await?;
    let task_b = fx.insert_task(TaskState::Submitted).await?;

    let msg = format!("shared_{}", fx.tag);
    insert_message(&fx, task_a.as_str(), &msg, "user", 1).await?;
    insert_message(&fx, task_b.as_str(), &msg, "user", 1).await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn duplicate_sequence_number_is_rejected() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;

    insert_message(&fx, task_id.as_str(), &format!("m1_{}", fx.tag), "user", 1).await?;
    insert_message(&fx, task_id.as_str(), &format!("m2_{}", fx.tag), "agent", 2).await?;

    let err = insert_message(&fx, task_id.as_str(), &format!("m3_{}", fx.tag), "user", 2)
        .await
        .expect_err("duplicate sequence number must be rejected");
    let s = format!("{err}").to_lowercase();
    assert!(
        s.contains("unique") || s.contains("duplicate"),
        "expected uniqueness error, got {s}"
    );

    fx.cleanup().await?;
    Ok(())
}
