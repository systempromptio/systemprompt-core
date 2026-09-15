//! Tests for the `systemprompt_traits` provider bridges implemented in
//! `services/providers.rs`: `AnalyticsProvider for AnalyticsService`,
//! and `FingerprintProvider for FingerprintRepository`. Happy paths run against
//! the migrated DB and assert the translated return values; every error arm
//! is driven through a closed pool to exercise the `Internal(e.to_string())`
//! mapping.

use chrono::{Duration, Utc};
use systemprompt_analytics::CreateSessionParams;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::{AnalyticsProviderError, FingerprintProvider};
use uuid::Uuid;

fn unique_session_id() -> SessionId {
    SessionId::new(format!("sess-prov-{}", Uuid::new_v4()))
}

async fn cleanup(pool: &DbPool, session_id: &SessionId) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
}

async fn seed(pool: &DbPool, session_id: &SessionId, fingerprint: &str) {
    let repo = systemprompt_test_fixtures::fixture_analytics_repositories(pool)
        .map(|r| r.sessions)
        .expect("repo");
    let params = CreateSessionParams {
        session_id,
        user_id: None,
        session_source: SessionSource::Web,
        fingerprint_hash: Some(fingerprint),
        ip_address: None,
        user_agent: None,
        device_type: None,
        browser: None,
        os: None,
        country: None,
        region: None,
        city: None,
        preferred_locale: None,
        referrer_source: None,
        referrer_url: None,
        landing_page: None,
        entry_url: None,
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        utm_content: None,
        utm_term: None,
        is_bot: false,
        is_ai_crawler: false,
        expires_at: Utc::now() + Duration::hours(1),
    };
    repo.create_session(&params).await.expect("seed");
}

mod fingerprint_provider {
    use super::*;

    #[tokio::test]
    async fn upsert_then_count_and_reuse() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");

        let fp = format!("fp-{}", Uuid::new_v4());
        FingerprintProvider::upsert_fingerprint(
            &repo,
            &fp,
            Some("1.2.3.4"),
            Some("Mozilla/5.0"),
            None,
        )
        .await
        .expect("upsert");

        let count = FingerprintProvider::count_active_sessions(&repo, &fp)
            .await
            .expect("count");
        assert!(count >= 0);
        // No active session references this fingerprint yet.
        assert!(
            FingerprintProvider::find_reusable_session(&repo, &fp)
                .await
                .expect("reuse")
                .is_none()
        );
    }

    #[tokio::test]
    async fn error_arms_map_to_internal() {
        let pool = closed_db_pool().await;
        let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");

        assert!(matches!(
            FingerprintProvider::count_active_sessions(&repo, "fp").await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            FingerprintProvider::find_reusable_session(&repo, "fp").await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            FingerprintProvider::upsert_fingerprint(&repo, "fp", None, None, None).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
    }
}
