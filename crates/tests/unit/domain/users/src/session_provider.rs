//! SessionProvider owner persistence and error translation.

use chrono::{Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::session_store::CreateSessionParams;
use systemprompt_traits::{
    AnalyticsProviderError, CreateSessionInput, SessionAnalytics as TraitSessionAnalytics,
    SessionProvider,
};
use systemprompt_users::SessionRepository;
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
    async fn authenticated_mcp_session_preserves_classification_and_analytics() {
        let database =
            systemprompt_test_fixtures::DisposableDb::installed("classified_session_roundtrip")
                .await
                .expect("private users database");
        let pool = database.pool().await.expect("users pool");
        let raw = pool.write_pool_arc().expect("write pool");
        let owner = UserId::new(format!("owner-{}", Uuid::new_v4().simple()));
        systemprompt_test_fixtures::seed_user_row(
            &pool,
            &owner,
            &format!("owner-{}@session.invalid", Uuid::new_v4().simple()),
        )
        .await
        .expect("seed session owner");
        let repository = SessionRepository::new(&pool).expect("session repository");
        let authenticated = unique_session_id();
        let anonymous = unique_session_id();
        let authenticated_expiry = chrono::TimeZone::timestamp_opt(&Utc, 1_900_000_000, 0)
            .single()
            .expect("fixed authenticated expiry");
        let anonymous_expiry = chrono::TimeZone::timestamp_opt(&Utc, 1_900_003_600, 0)
            .single()
            .expect("fixed anonymous expiry");
        let authenticated_analytics = TraitSessionAnalytics {
            fingerprint_hash: Some("authenticated-fingerprint".to_owned()),
            user_agent: Some("classified-mcp-agent".to_owned()),
            preferred_locale: Some("en-GB".to_owned()),
            ..TraitSessionAnalytics::default()
        };
        let anonymous_analytics = TraitSessionAnalytics {
            fingerprint_hash: Some("anonymous-fingerprint".to_owned()),
            user_agent: Some("anonymous-web-agent".to_owned()),
            ..TraitSessionAnalytics::default()
        };

        SessionProvider::create_session(
            &repository,
            CreateSessionInput::new(
                &authenticated,
                &authenticated_analytics,
                SessionSource::Mcp,
                authenticated_expiry,
            )
            .with_user_id(&owner)
            .with_classification(true, true),
        )
        .await
        .expect("authenticated MCP session");
        SessionProvider::create_session(
            &repository,
            CreateSessionInput::new(
                &anonymous,
                &anonymous_analytics,
                SessionSource::Web,
                anonymous_expiry,
            ),
        )
        .await
        .expect("anonymous default session");

        let rows: Vec<(
            String,
            Option<String>,
            String,
            bool,
            bool,
            Option<String>,
            Option<String>,
            Option<String>,
            chrono::DateTime<Utc>,
        )> = sqlx::query_as(
            "SELECT session_id, user_id, session_source, is_bot, is_ai_crawler, \
             fingerprint_hash, user_agent, preferred_locale, expires_at FROM user_sessions \
             WHERE session_id = ANY($1) ORDER BY session_id",
        )
        .bind(vec![authenticated.to_string(), anonymous.to_string()])
        .fetch_all(raw.as_ref())
        .await
        .expect("persisted sessions");
        assert_eq!(rows.len(), 2);
        let classified = rows
            .iter()
            .find(|row| row.0 == authenticated.as_str())
            .unwrap();
        assert_eq!(classified.1.as_deref(), Some(owner.as_str()));
        assert_eq!(classified.2, "mcp");
        assert!(classified.3);
        assert!(classified.4);
        assert_eq!(classified.5.as_deref(), Some("authenticated-fingerprint"));
        assert_eq!(classified.6.as_deref(), Some("classified-mcp-agent"));
        assert_eq!(classified.7.as_deref(), Some("en-GB"));
        assert_eq!(classified.8, authenticated_expiry);
        let defaulted = rows.iter().find(|row| row.0 == anonymous.as_str()).unwrap();
        assert!(
            defaulted.1.is_none(),
            "owner must not bleed into anonymous session"
        );
        assert_eq!(defaulted.2, "web");
        assert!(
            !defaulted.3,
            "bot classification must not bleed between sessions"
        );
        assert!(
            !defaulted.4,
            "crawler classification must not bleed between sessions"
        );
        assert_eq!(defaulted.5.as_deref(), Some("anonymous-fingerprint"));
        assert_eq!(defaulted.6.as_deref(), Some("anonymous-web-agent"));
        assert!(defaulted.7.is_none());
        assert_eq!(defaulted.8, anonymous_expiry);

        raw.close().await;
        drop(raw);
        drop(repository);
        drop(pool);
        database.drop_now().await;
    }

    #[tokio::test]
    async fn create_and_find_session_by_id_translates_row() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let service = SessionRepository::new(&pool).expect("sessions");

        let sid = unique_session_id();
        let fp = format!("fp-{}", Uuid::new_v4());
        let analytics = TraitSessionAnalytics {
            fingerprint_hash: Some(fp.clone()),
            ..TraitSessionAnalytics::default()
        };
        SessionProvider::create_session(
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
        let service = SessionRepository::new(&pool).expect("sessions");

        let sid = unique_session_id();
        let fp = format!("fp-{}", Uuid::new_v4());
        seed(&pool, &sid, &fp).await;

        let recent = SessionProvider::find_recent_session_by_fingerprint(&service, &fp, 3_600)
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
        let service = SessionRepository::new(&pool).expect("sessions");

        let sid = unique_session_id();
        seed(&pool, &sid, &format!("fp-{}", Uuid::new_v4())).await;

        service.mark_session_converted(&sid).await.expect("convert");
        SessionProvider::revoke_session(&service, &sid)
            .await
            .expect("revoke");

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
        let service = SessionRepository::new(&pool).expect("sessions");
        let sid = unique_session_id();
        let user = UserId::new("u".to_owned());
        let analytics = TraitSessionAnalytics::default();

        assert!(matches!(
            SessionProvider::create_session(
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
            SessionProvider::find_recent_session_by_fingerprint(&service, "fp", 60).await,
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
            SessionProvider::revoke_session(&service, &sid).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.revoke_all_sessions_for_user(&user).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            SessionProvider::migrate_user_sessions(&service, &user, &user).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            service.mark_session_converted(&sid).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
    }
}
