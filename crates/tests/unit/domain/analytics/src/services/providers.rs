//! Tests for the `systemprompt_traits` provider bridges implemented in
//! `services/providers.rs`: `AnalyticsProvider for AnalyticsService`,
//! `FingerprintProvider for FingerprintRepository`, and
//! `SessionAnalyticsProvider for SessionRepository`. Happy paths run against
//! the migrated DB and assert the translated return values; every error arm
//! is driven through a closed pool to exercise the `Internal(e.to_string())`
//! mapping.

use chrono::{Duration, Utc};
use systemprompt_analytics::{
    AnalyticsService, CreateSessionParams, FingerprintRepository, SessionRepository,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::{
    AnalyticsProvider, AnalyticsProviderError, CreateSessionInput, FingerprintProvider,
    SessionAnalytics as TraitSessionAnalytics, SessionAnalyticsProvider,
    SessionAnalyticsProviderError,
};
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
    let repo = SessionRepository::new(pool).expect("repo");
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

mod analytics_provider {
    use super::*;

    #[tokio::test]
    async fn create_and_find_session_by_id_translates_row() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let service = AnalyticsService::new(&pool, None, None).expect("service");

        let sid = unique_session_id();
        let fp = format!("fp-{}", Uuid::new_v4());
        let analytics = TraitSessionAnalytics {
            fingerprint_hash: Some(fp.clone()),
            ..TraitSessionAnalytics::default()
        };
        AnalyticsProvider::create_session(
            &service,
            CreateSessionInput {
                session_id: &sid,
                user_id: None,
                analytics: &analytics,
                session_source: SessionSource::Web,
                is_bot: false,
                is_ai_crawler: false,
                expires_at: Utc::now() + Duration::hours(1),
            },
        )
        .await
        .expect("create");

        let found = service
            .find_session_by_id(&sid)
            .await
            .expect("find")
            .expect("present");
        assert_eq!(found.session_id.as_str(), sid.as_str());
        assert_eq!(found.fingerprint.as_deref(), Some(fp.as_str()));

        let active = service
            .find_active_session_by_id(&sid)
            .await
            .expect("find active")
            .expect("present");
        assert!(active.user_id.is_none());

        cleanup(&pool, &sid).await;
    }

    #[tokio::test]
    async fn find_recent_by_fingerprint_returns_created_session() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let service = AnalyticsService::new(&pool, None, None).expect("service");

        let sid = unique_session_id();
        let fp = format!("fp-{}", Uuid::new_v4());
        seed(&pool, &sid, &fp).await;

        let recent = AnalyticsProvider::find_recent_session_by_fingerprint(&service, &fp, 3_600)
            .await
            .expect("recent")
            .expect("present");
        assert_eq!(recent.session_id.as_str(), sid.as_str());
        assert_eq!(recent.fingerprint.as_deref(), Some(fp.as_str()));

        cleanup(&pool, &sid).await;
    }

    #[tokio::test]
    async fn revoke_convert_and_user_scoped_ops_succeed() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let service = AnalyticsService::new(&pool, None, None).expect("service");

        let sid = unique_session_id();
        seed(&pool, &sid, &format!("fp-{}", Uuid::new_v4())).await;

        service.mark_session_converted(&sid).await.expect("convert");
        service.revoke_session(&sid).await.expect("revoke");

        let user = UserId::new(format!("user-{}", Uuid::new_v4()));
        let other = UserId::new(format!("user-{}", Uuid::new_v4()));
        assert_eq!(
            service
                .revoke_all_sessions_for_user(&user)
                .await
                .expect("revoke all"),
            0
        );
        assert_eq!(
            service
                .migrate_user_sessions(&user, &other)
                .await
                .expect("migrate"),
            0
        );

        cleanup(&pool, &sid).await;
    }

    #[tokio::test]
    async fn every_analytics_error_arm_maps_to_internal() {
        let pool = closed_db_pool().await;
        let service = AnalyticsService::new(&pool, None, None).expect("service");
        let sid = unique_session_id();
        let user = UserId::new("u".to_owned());
        let analytics = TraitSessionAnalytics::default();

        assert!(matches!(
            AnalyticsProvider::create_session(
                &service,
                CreateSessionInput {
                    session_id: &sid,
                    user_id: None,
                    analytics: &analytics,
                    session_source: SessionSource::Web,
                    is_bot: false,
                    is_ai_crawler: false,
                    expires_at: Utc::now(),
                },
            )
            .await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            AnalyticsProvider::find_recent_session_by_fingerprint(&service, "fp", 60).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.find_session_by_id(&sid).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.find_active_session_by_id(&sid).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.revoke_session(&sid).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.revoke_all_sessions_for_user(&user).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.migrate_user_sessions(&user, &user).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.mark_session_converted(&sid).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
    }
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
        let repo = FingerprintRepository::new(&pool).expect("repo");

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
        let repo = FingerprintRepository::new(&pool).expect("repo");

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

mod session_analytics_provider {
    use super::*;

    #[tokio::test]
    async fn increment_counters_through_trait() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let repo = SessionRepository::new(&pool).expect("repo");

        let sid = unique_session_id();
        seed(&pool, &sid, &format!("fp-{}", Uuid::new_v4())).await;

        SessionAnalyticsProvider::increment_task_count(&repo, &sid)
            .await
            .expect("task");
        SessionAnalyticsProvider::increment_message_count(&repo, &sid)
            .await
            .expect("msg");

        let s = repo.find_by_id(&sid).await.expect("find").expect("present");
        assert_eq!(s.task_count, Some(1));
        assert_eq!(s.message_count, Some(1));

        cleanup(&pool, &sid).await;
    }

    #[tokio::test]
    async fn increment_error_arms_map_to_internal() {
        let pool = closed_db_pool().await;
        let repo = SessionRepository::new(&pool).expect("repo");
        let sid = unique_session_id();

        assert!(matches!(
            SessionAnalyticsProvider::increment_task_count(&repo, &sid).await,
            Err(SessionAnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            SessionAnalyticsProvider::increment_message_count(&repo, &sid).await,
            Err(SessionAnalyticsProviderError::Internal(_))
        ));
    }
}
