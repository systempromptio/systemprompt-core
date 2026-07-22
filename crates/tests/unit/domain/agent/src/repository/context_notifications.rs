// DB-backed tests for ContextNotificationRepository: inserting a notification
// returns its serial id with broadcasted=false, mark_broadcasted flips the
// flag, and a notification_type outside the CHECK constraint is rejected.

use serde_json::json;
use systemprompt_agent::repository::context::ContextNotificationRepository;
use systemprompt_identifiers::{AgentId, ContextId};

use super::{repos, seed_context_and_task, seed_user_and_session, try_pool};

#[tokio::test]
async fn insert_persists_row_and_mark_broadcasted_flips_flag() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, _task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let repo = ContextNotificationRepository::new(&pool).expect("repository");
    let agent_id = AgentId::new("ctx_notif_agent");

    let id = repo
        .insert(
            &context_id,
            &agent_id,
            "notifications/taskStatusUpdate",
            &json!({"state": "working"}),
        )
        .await
        .expect("insert notification");

    let pg = pool.pool_arc().expect("pg pool");
    let (stored_type, broadcasted): (String, bool) = sqlx::query_as(
        "SELECT notification_type, broadcasted FROM context_notifications WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pg.as_ref())
    .await
    .expect("fetch inserted row");
    assert_eq!(stored_type, "notifications/taskStatusUpdate");
    assert!(!broadcasted, "new notifications start un-broadcasted");

    repo.mark_broadcasted(id).await.expect("mark broadcasted");

    let (broadcasted,): (bool,) =
        sqlx::query_as("SELECT broadcasted FROM context_notifications WHERE id = $1")
            .bind(id)
            .fetch_one(pg.as_ref())
            .await
            .expect("fetch updated row");
    assert!(broadcasted);
}

#[tokio::test]
async fn insert_rejects_unknown_notification_type() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repo = ContextNotificationRepository::new(&pool).expect("repository");

    let err = repo
        .insert(
            &ContextId::generate(),
            &AgentId::new("ctx_notif_agent"),
            "notifications/unsupported",
            &json!({}),
        )
        .await
        .expect_err("CHECK constraint must reject unknown notification types");
    assert!(
        err.to_string().to_lowercase().contains("check"),
        "got {err}"
    );
}

#[tokio::test]
async fn mark_broadcasted_on_unknown_id_is_a_no_op() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repo = ContextNotificationRepository::new(&pool).expect("repository");
    repo.mark_broadcasted(i32::MIN)
        .await
        .expect("updating an absent row must not error");
}
