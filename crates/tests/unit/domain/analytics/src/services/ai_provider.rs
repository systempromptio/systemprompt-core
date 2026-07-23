//! Tests for `AnalyticsAiSessionProvider` — the `AiSessionProvider` bridge
//! over `SessionRepository`. Happy paths drive the real DB; the error arms
//! are driven through a closed pool so the `AiProviderError::Internal`
//! translation is exercised.

use chrono::{Duration, Utc};
use systemprompt_analytics::{AnalyticsAiSessionProvider, SessionRepository};
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::{AiProviderError, AiSessionProvider, CreateAiSessionParams};
use uuid::Uuid;

fn unique_session_id() -> SessionId {
    SessionId::new(format!("sess-ai-{}", Uuid::new_v4()))
}

async fn cleanup(pool: &systemprompt_database::DbPool, session_id: &SessionId) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
}

#[tokio::test]
async fn create_session_then_increment_usage_round_trip() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let provider = AnalyticsAiSessionProvider::new(&pool).expect("provider");

    let sid = unique_session_id();
    provider
        .create_session(CreateAiSessionParams {
            session_id: &sid,
            user_id: None,
            session_source: SessionSource::Cli,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("create");

    provider
        .increment_ai_usage(&sid, 120, 6_000)
        .await
        .expect("usage");

    let session = SessionRepository::new(&pool)
        .expect("repo")
        .find_by_id(&sid)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(session.ai_request_count, Some(1));

    cleanup(&pool, &sid).await;
}

#[tokio::test]
async fn from_repository_shares_the_backing_repo() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");
    let provider = AnalyticsAiSessionProvider::from_repository(repo);

    let sid = unique_session_id();
    provider
        .create_session(CreateAiSessionParams {
            session_id: &sid,
            user_id: None,
            session_source: SessionSource::Web,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("create");
    assert!(
        SessionRepository::new(&pool)
            .expect("repo")
            .find_by_id(&sid)
            .await
            .expect("find")
            .is_some(),
        "created session must be visible through the shared repo"
    );

    cleanup(&pool, &sid).await;
}

#[tokio::test]
async fn create_session_maps_pool_failure_to_internal() {
    let pool = closed_db_pool().await;
    let provider = AnalyticsAiSessionProvider::new(&pool).expect("provider");

    let err = provider
        .create_session(CreateAiSessionParams {
            session_id: &unique_session_id(),
            user_id: None,
            session_source: SessionSource::Cli,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect_err("closed pool must fail");
    assert!(matches!(err, AiProviderError::Internal(_)));
}

#[tokio::test]
async fn increment_ai_usage_maps_pool_failure_to_internal() {
    let pool = closed_db_pool().await;
    let provider = AnalyticsAiSessionProvider::new(&pool).expect("provider");

    let err = provider
        .increment_ai_usage(&unique_session_id(), 10, 500)
        .await
        .expect_err("closed pool must fail");
    assert!(matches!(err, AiProviderError::Internal(_)));
}
