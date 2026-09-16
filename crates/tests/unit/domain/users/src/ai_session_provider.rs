//! Tests for `UsersAiSessionProvider` — the `AiSessionProvider` bridge
//! over `SessionRepository`. Happy paths drive the real DB; the error arms
//! are driven through a closed pool so the `AiProviderError::Internal`
//! translation is exercised.

use chrono::{Duration, Utc};
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::{AiProviderError, AiSessionProvider, CreateAiSessionParams};
use systemprompt_users::{SessionRepository, UsersAiSessionProvider};
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
    let provider = UsersAiSessionProvider::from_repository(
        SessionRepository::new(&pool).expect("session repository"),
    );

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
    let provider = UsersAiSessionProvider::from_repository(repo);

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
    let provider = UsersAiSessionProvider::from_repository(
        SessionRepository::new(&pool).expect("session repository"),
    );

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
    let provider = UsersAiSessionProvider::from_repository(
        SessionRepository::new(&pool).expect("session repository"),
    );

    let err = provider
        .increment_ai_usage(&unique_session_id(), 10, 500)
        .await
        .expect_err("closed pool must fail");
    assert!(matches!(err, AiProviderError::Internal(_)));
}

#[tokio::test]
async fn find_live_session_reports_only_a_live_session() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("session repository");
    let provider = UsersAiSessionProvider::from_repository(
        SessionRepository::new(&pool).expect("session repository"),
    );

    let live = unique_session_id();
    let expired = unique_session_id();
    let revoked = unique_session_id();
    for (sid, expires_at) in [
        (&live, Utc::now() + Duration::hours(1)),
        (&expired, Utc::now() - Duration::minutes(1)),
        (&revoked, Utc::now() + Duration::hours(1)),
    ] {
        provider
            .create_session(CreateAiSessionParams {
                session_id: sid,
                user_id: None,
                session_source: SessionSource::Cli,
                expires_at,
            })
            .await
            .expect("create");
    }
    repo.revoke_session(&revoked).await.expect("revoke");

    assert!(
        provider
            .find_live_session(&live)
            .await
            .expect("lookup")
            .is_some(),
        "an unexpired, unrevoked session is live"
    );
    assert!(
        provider
            .find_live_session(&expired)
            .await
            .expect("lookup")
            .is_none(),
        "an expired session is not live"
    );
    assert!(
        provider
            .find_live_session(&revoked)
            .await
            .expect("lookup")
            .is_none(),
        "a revoked session is not live"
    );
    assert!(
        provider
            .find_live_session(&unique_session_id())
            .await
            .expect("lookup")
            .is_none()
    );

    for sid in [&live, &expired, &revoked] {
        cleanup(&pool, sid).await;
    }
}

#[tokio::test]
async fn find_live_session_maps_pool_failure_to_internal() {
    let pool = closed_db_pool().await;
    let provider = UsersAiSessionProvider::from_repository(
        SessionRepository::new(&pool).expect("session repository"),
    );

    let err = provider
        .find_live_session(&unique_session_id())
        .await
        .expect_err("closed pool must fail");
    assert!(matches!(err, AiProviderError::Internal(_)));
}
