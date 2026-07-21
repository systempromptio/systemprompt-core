use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use http::{HeaderMap, HeaderValue, Uri};
use sqlx::{PgPool, Row};
use systemprompt_analytics::{AnalyticsService, SessionAnalytics};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_models::ContentRouting;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_traits::{CreateSessionInput, ExtractSignals};
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use uuid::Uuid;

static SERIAL: OnceCell<Mutex<()>> = OnceCell::const_new();

async fn acquire_serial() -> MutexGuard<'static, ()> {
    SERIAL
        .get_or_init(|| async { Mutex::new(()) })
        .await
        .lock()
        .await
}

struct HtmlRouting;

impl ContentRouting for HtmlRouting {
    fn is_html_page(&self, _path: &str) -> bool {
        true
    }

    fn determine_source(&self, _path: &str) -> String {
        "web".to_owned()
    }
}

struct Fixture {
    pool: PgPool,
    db: DbPool,
    tag: String,
    session_ids: Vec<SessionId>,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url = fixture_database_url()?;
        let db = fixture_db_pool(&url).await?;
        let pool = db.pool_arc()?.as_ref().clone();
        let tag = Uuid::new_v4().simple().to_string();
        Ok(Self {
            pool,
            db,
            tag,
            session_ids: Vec::new(),
            _guard: guard,
        })
    }

    fn service(&self) -> Result<AnalyticsService> {
        Ok(AnalyticsService::new(
            &self.db,
            None,
            Some(Arc::new(HtmlRouting)),
        )?)
    }

    fn user_agent(&self, suffix: &str) -> String {
        format!(
            "Mozilla/5.0 (X11; Linux x86_64) Pipeline-{}-{suffix}/1.0",
            self.tag
        )
    }

    fn headers(user_agent: &str, referer: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_str(user_agent).unwrap());
        headers.insert("accept-language", HeaderValue::from_static("en-US,en"));
        if let Some(referer) = referer {
            headers.insert("referer", HeaderValue::from_str(referer).unwrap());
        }
        headers
    }

    async fn create_session(
        &mut self,
        service: &AnalyticsService,
        analytics: &SessionAnalytics,
        expires_at: DateTime<Utc>,
    ) -> Result<SessionId> {
        let session_id = SessionId::generate();
        service
            .create_analytics_session(CreateSessionInput {
                session_id: &session_id,
                user_id: None,
                analytics,
                session_source: SessionSource::Web,
                is_bot: analytics.is_bot,
                is_ai_crawler: false,
                expires_at,
            })
            .await?;
        self.session_ids.push(session_id.clone());
        Ok(session_id)
    }

    async fn fetch_row(&self, session_id: &SessionId) -> Result<sqlx::postgres::PgRow> {
        Ok(sqlx::query(
            "SELECT fingerprint_hash, user_agent, landing_page, entry_url, referrer_url, \
             referrer_source, utm_source, utm_medium, utm_campaign, utm_content, utm_term, \
             is_bot, request_count, started_at, last_activity_at, duration_seconds FROM \
             user_sessions WHERE session_id = $1",
        )
        .bind(session_id.as_str())
        .fetch_one(&self.pool)
        .await?)
    }

    async fn cleanup(&self) -> Result<()> {
        for session_id in &self.session_ids {
            let _ = sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
                .bind(session_id.as_str())
                .execute(&self.pool)
                .await;
        }
        Ok(())
    }
}

fn expires_in_one_hour() -> DateTime<Utc> {
    Utc::now() + Duration::hours(1)
}

#[tokio::test]
async fn create_session_persists_extracted_attribution() -> Result<()> {
    let mut fx = Fixture::new().await?;
    let service = fx.service()?;
    let ua = fx.user_agent("attr");
    let headers = Fixture::headers(&ua, Some("https://google.com/search?q=test"));
    let uri: Uri = "https://example.com/?utm_source=google&utm_medium=organic&utm_campaign=product-launch&utm_content=hero&utm_term=agents"
        .parse()?;

    let analytics = service.extract_analytics(
        &headers,
        ExtractSignals {
            uri: Some(&uri),
            ..Default::default()
        },
    );
    assert!(!analytics.is_bot);

    let session_id = fx
        .create_session(&service, &analytics, expires_in_one_hour())
        .await?;

    let row = fx.fetch_row(&session_id).await?;
    assert_eq!(
        row.get::<Option<String>, _>("fingerprint_hash"),
        Some(analytics.compute_fingerprint())
    );
    assert_eq!(row.get::<Option<String>, _>("user_agent"), Some(ua));
    assert_eq!(
        row.get::<Option<String>, _>("landing_page"),
        Some("/".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("entry_url"),
        Some(uri.to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("referrer_url"),
        Some("https://google.com/search?q=test".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("referrer_source"),
        Some("google.com".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("utm_source"),
        Some("google".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("utm_medium"),
        Some("organic".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("utm_campaign"),
        Some("product-launch".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("utm_content"),
        Some("hero".to_owned())
    );
    assert_eq!(
        row.get::<Option<String>, _>("utm_term"),
        Some("agents".to_owned())
    );
    assert!(!row.get::<bool, _>("is_bot"));

    fx.cleanup().await
}

#[tokio::test]
async fn recent_fingerprint_lookup_deduplicates_sessions() -> Result<()> {
    let mut fx = Fixture::new().await?;
    let service = fx.service()?;
    let headers = Fixture::headers(&fx.user_agent("dedup"), None);
    let uri: Uri = "https://example.com/".parse()?;

    let analytics = service.extract_analytics(
        &headers,
        ExtractSignals {
            uri: Some(&uri),
            ..Default::default()
        },
    );
    let fingerprint = analytics.compute_fingerprint();
    let session_id = fx
        .create_session(&service, &analytics, expires_in_one_hour())
        .await?;

    let found = service
        .find_recent_session_by_fingerprint(&fingerprint, 3600)
        .await?;
    assert_eq!(found.map(|record| record.session_id), Some(session_id));

    let other_headers = Fixture::headers(&fx.user_agent("dedup-other"), None);
    let other_analytics = service.extract_analytics(
        &other_headers,
        ExtractSignals {
            uri: Some(&uri),
            ..Default::default()
        },
    );
    let other_fingerprint = other_analytics.compute_fingerprint();
    assert_ne!(fingerprint, other_fingerprint);
    assert!(
        service
            .find_recent_session_by_fingerprint(&other_fingerprint, 3600)
            .await?
            .is_none()
    );

    fx.cleanup().await
}

#[tokio::test]
async fn ended_session_excluded_from_fingerprint_dedup() -> Result<()> {
    let mut fx = Fixture::new().await?;
    let service = fx.service()?;
    let headers = Fixture::headers(&fx.user_agent("ended"), None);
    let uri: Uri = "https://example.com/".parse()?;

    let analytics = service.extract_analytics(
        &headers,
        ExtractSignals {
            uri: Some(&uri),
            ..Default::default()
        },
    );
    let fingerprint = analytics.compute_fingerprint();
    let session_id = fx
        .create_session(&service, &analytics, expires_in_one_hour())
        .await?;

    service.session_repo().end_session(&session_id).await?;

    assert!(
        service
            .find_recent_session_by_fingerprint(&fingerprint, 3600)
            .await?
            .is_none()
    );

    fx.cleanup().await
}

#[tokio::test]
async fn bot_user_agent_session_marked_is_bot() -> Result<()> {
    let mut fx = Fixture::new().await?;
    let service = fx.service()?;
    for bot_ua in ["Go-http-client/2.0", "Googlebot/2.1"] {
        let headers = Fixture::headers(bot_ua, None);
        let uri: Uri = "https://example.com/".parse()?;
        let analytics = service.extract_analytics(
            &headers,
            ExtractSignals {
                uri: Some(&uri),
                ..Default::default()
            },
        );
        assert!(analytics.is_bot, "{bot_ua} not a bot");

        let session_id = fx
            .create_session(&service, &analytics, expires_in_one_hour())
            .await?;
        let row = fx.fetch_row(&session_id).await?;
        assert!(
            row.get::<bool, _>("is_bot"),
            "{bot_ua} not persisted as bot"
        );
    }

    fx.cleanup().await
}

#[tokio::test]
async fn request_count_and_activity_tracking() -> Result<()> {
    let mut fx = Fixture::new().await?;
    let service = fx.service()?;
    let headers = Fixture::headers(&fx.user_agent("count"), None);
    let uri: Uri = "https://example.com/".parse()?;

    let analytics = service.extract_analytics(
        &headers,
        ExtractSignals {
            uri: Some(&uri),
            ..Default::default()
        },
    );
    let session_id = fx
        .create_session(&service, &analytics, expires_in_one_hour())
        .await?;

    let initial = fx.fetch_row(&session_id).await?;
    let initial_count = initial.get::<i32, _>("request_count");
    let started_at = initial.get::<DateTime<Utc>, _>("started_at");

    for _ in 0..3 {
        service
            .session_repo()
            .increment_request_count(&session_id)
            .await?;
    }
    service.session_repo().update_activity(&session_id).await?;

    let updated = fx.fetch_row(&session_id).await?;
    assert_eq!(updated.get::<i32, _>("request_count"), initial_count + 3);
    assert_eq!(updated.get::<DateTime<Utc>, _>("started_at"), started_at);
    assert!(updated.get::<DateTime<Utc>, _>("last_activity_at") >= started_at);
    assert!(updated.get::<Option<i32>, _>("duration_seconds").is_some());

    fx.cleanup().await
}

#[tokio::test]
async fn create_session_upserts_on_duplicate_id() -> Result<()> {
    let mut fx = Fixture::new().await?;
    let service = fx.service()?;
    let headers = Fixture::headers(&fx.user_agent("upsert"), None);
    let uri: Uri = "https://example.com/".parse()?;

    let analytics = service.extract_analytics(
        &headers,
        ExtractSignals {
            uri: Some(&uri),
            ..Default::default()
        },
    );
    let session_id = fx
        .create_session(&service, &analytics, expires_in_one_hour())
        .await?;

    let later = Utc::now() + Duration::hours(2);
    service
        .create_analytics_session(CreateSessionInput {
            session_id: &session_id,
            user_id: None,
            analytics: &analytics,
            session_source: SessionSource::Web,
            is_bot: false,
            is_ai_crawler: false,
            expires_at: later,
        })
        .await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_sessions WHERE session_id = $1")
        .bind(session_id.as_str())
        .fetch_one(&fx.pool)
        .await?;
    assert_eq!(count, 1);

    let expires_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT expires_at FROM user_sessions WHERE session_id = $1")
            .bind(session_id.as_str())
            .fetch_one(&fx.pool)
            .await?;
    let expires_at = expires_at.expect("expires_at persisted");
    assert!((expires_at - later).num_seconds().abs() < 5);

    fx.cleanup().await
}
