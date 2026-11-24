//! `TaskRepository::list_tasks` pages the caller's tasks by keyset: the first
//! page is the newest rows, and a page read after more tasks were created
//! resumes strictly after the cursor, so nothing repeats or is skipped.

use super::{repos, seed_context_and_task, seed_user_and_session};
use systemprompt_agent::repository::task::ListTasksFilter;
use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_test_fixtures::test_db_pool;

fn mine(user_id: &UserId) -> ListTasksFilter<'_> {
    ListTasksFilter {
        user_id,
        context_id: None,
        status: None,
        status_timestamp_after: None,
    }
}

fn ids(tasks: &[systemprompt_agent::models::a2a::Task]) -> Vec<TaskId> {
    tasks.iter().map(|task| task.id.clone()).collect()
}

#[tokio::test]
async fn a_page_read_after_inserts_resumes_after_its_cursor() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    for _ in 0..3 {
        seed_context_and_task(&r, &owner, &session).await;
    }
    let (stranger, stranger_session) = seed_user_and_session(&pool).await;
    let (_, foreign) = seed_context_and_task(&r, &stranger, &stranger_session).await;

    let everything = r
        .tasks
        .list_tasks(mine(&owner), None, 10)
        .await
        .expect("full listing");
    assert_eq!(everything.items.len(), 3);
    assert!(everything.next_cursor.is_none());
    assert!(!ids(&everything.items).contains(&foreign));

    let first = r
        .tasks
        .list_tasks(mine(&owner), None, 2)
        .await
        .expect("first page");
    assert_eq!(ids(&first.items), ids(&everything.items)[..2].to_vec());
    let cursor = first.next_cursor.expect("a further page exists");

    let (_, newer) = seed_context_and_task(&r, &owner, &session).await;

    let second = r
        .tasks
        .list_tasks(mine(&owner), Some(&cursor), 2)
        .await
        .expect("second page");
    assert_eq!(ids(&second.items), ids(&everything.items)[2..].to_vec());
    assert!(second.next_cursor.is_none());
    assert!(!ids(&second.items).contains(&newer));

    let decoded = cursor.to_string().parse().expect("cursor round-trips");
    assert_eq!(cursor, decoded);
}

#[tokio::test]
async fn a_cursor_never_reaches_another_users_tasks() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let (stranger, stranger_session) = seed_user_and_session(&pool).await;
    seed_context_and_task(&r, &owner, &session).await;
    seed_context_and_task(&r, &owner, &session).await;
    seed_context_and_task(&r, &stranger, &stranger_session).await;

    let first = r
        .tasks
        .list_tasks(mine(&owner), None, 1)
        .await
        .expect("first page");
    let cursor = first.next_cursor.expect("cursor");
    let foreign = r
        .tasks
        .list_tasks(mine(&stranger), Some(&cursor), 10)
        .await
        .expect("stranger page");
    assert!(
        foreign
            .items
            .iter()
            .all(|task| !ids(&first.items).contains(&task.id))
    );
}
