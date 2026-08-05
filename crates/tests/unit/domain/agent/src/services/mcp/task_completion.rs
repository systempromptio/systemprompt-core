// complete_task webhook broadcast: an unknown task short-circuits before any
// HTTP call, and a real task drives the broadcast attempt against the fixture
// API URL (nothing listens there, so the delivery-failure branch is taken and
// swallowed — completion never fails the task).

use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::mcp::task_helper::complete_task;
use systemprompt_identifiers::TaskId;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

#[tokio::test]
async fn unknown_task_completes_without_broadcast() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    complete_task(&task_repo, &TaskId::generate(), "test-token")
        .await
        .expect("unknown task is a no-op");
}

#[tokio::test]
async fn known_task_swallows_webhook_delivery_failure() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (_ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    complete_task(&task_repo, &task_id, "test-token")
        .await
        .expect("webhook failure must not fail completion");
}
