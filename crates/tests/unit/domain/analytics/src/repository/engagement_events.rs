//! DB-backed tests for `EngagementRepository`: event creation with default
//! and populated optional metrics, lookups by id/session/user, and the
//! per-session engagement summary aggregate.

use systemprompt_analytics::{
    CreateEngagementEventInput, EngagementOptionalMetrics, EngagementRepository,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{EngagementEventId, SessionId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn sample_input(page_url: &str) -> CreateEngagementEventInput {
    CreateEngagementEventInput {
        page_url: page_url.to_owned(),
        time_on_page_ms: 1500,
        max_scroll_depth: 80,
        click_count: 3,
        optional_metrics: EngagementOptionalMetrics {
            keyboard_events: Some(12),
            is_rage_click: Some(true),
            ..EngagementOptionalMetrics::default()
        },
        ..CreateEngagementEventInput::default()
    }
}

async fn cleanup(pool: &DbPool, session_id: &SessionId) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM engagement_events WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
}

#[tokio::test]
async fn create_then_lookup_by_id_session_and_user() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = EngagementRepository::new(&pool).expect("repo");

    let session_id = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    let user_id = UserId::new(format!("user-{}", Uuid::new_v4()));

    let id = repo
        .create_engagement(&session_id, &user_id, None, &sample_input("/docs"))
        .await
        .expect("create");

    let event = repo
        .find_by_id(&id)
        .await
        .expect("find_by_id")
        .expect("present");
    assert_eq!(event.session_id, session_id);
    assert_eq!(event.user_id, user_id);
    assert_eq!(event.page_url, "/docs");
    assert_eq!(event.time_on_page_ms, 1500);
    assert_eq!(event.max_scroll_depth, 80);
    assert_eq!(event.click_count, 3);
    assert_eq!(event.keyboard_events, Some(12));
    assert_eq!(event.is_rage_click, Some(true));
    assert_eq!(event.focus_time_ms, 0);

    let missing = EngagementEventId::generate();
    assert!(repo.find_by_id(&missing).await.expect("miss").is_none());

    let by_session = repo
        .list_by_session(&session_id)
        .await
        .expect("list_by_session");
    assert_eq!(by_session.len(), 1);

    let by_user = repo.list_by_user(&user_id, 10).await.expect("list_by_user");
    assert_eq!(by_user.len(), 1);
    assert_eq!(by_user[0].id, id);

    cleanup(&pool, &session_id).await;
}

#[tokio::test]
async fn session_engagement_summary_aggregates_pages() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = EngagementRepository::new(&pool).expect("repo");

    let session_id = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    let user_id = UserId::new(format!("user-{}", Uuid::new_v4()));

    let empty = repo
        .get_session_engagement_summary(&session_id)
        .await
        .expect("summary empty");
    assert!(empty.is_none());

    repo.create_engagement(&session_id, &user_id, None, &sample_input("/a"))
        .await
        .expect("create a");
    repo.create_engagement(&session_id, &user_id, None, &sample_input("/b"))
        .await
        .expect("create b");

    let summary = repo
        .get_session_engagement_summary(&session_id)
        .await
        .expect("summary")
        .expect("present");
    assert_eq!(summary.page_count, Some(2));
    assert_eq!(summary.total_time_on_page_ms, Some(3000));
    assert_eq!(summary.total_clicks, Some(6));
    assert_eq!(summary.rage_click_pages, Some(2));

    cleanup(&pool, &session_id).await;
}
