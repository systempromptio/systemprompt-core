// Corrupt stored task rows surface as a 500-class repository error on read
// (Decode for an unparsable value, InvalidData for a missing one) instead of
// being replaced with fresh default metadata or an empty agent name.

use super::{repos, seed_context_and_task, seed_user_and_session};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::RepositoryError;

#[tokio::test]
async fn unreadable_task_metadata_is_an_error() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let pg = pool.pool();
    sqlx::query(
        "UPDATE agent_tasks SET metadata = '{\"task_type\": 42}'::jsonb WHERE task_id = $1",
    )
    .bind(task_id.to_string())
    .execute(pg.as_ref())
    .await
    .expect("corrupt metadata");

    let err = r
        .tasks
        .get_task(&task_id)
        .await
        .expect_err("corrupt metadata must not read as defaults");
    assert!(matches!(err, RepositoryError::Decode { .. }), "got {err}");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn task_without_agent_name_is_an_error() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let pg = pool.pool();
    sqlx::query("UPDATE agent_tasks SET agent_name = NULL WHERE task_id = $1")
        .bind(task_id.to_string())
        .execute(pg.as_ref())
        .await
        .expect("clear agent_name");

    let err = r
        .tasks
        .get_task(&task_id)
        .await
        .expect_err("a task row without an agent must not read as agent \"\"");
    assert!(matches!(err, RepositoryError::InvalidData(_)), "got {err}");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn task_created_without_metadata_reads_back() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let task = r
        .tasks
        .get_task(&task_id)
        .await
        .expect("get")
        .expect("present");
    let metadata = task.metadata.expect("metadata projected from the row");
    assert_eq!(metadata.agent_name, "test-agent");

    r.tasks.delete_task(&task_id).await.ok();
}
