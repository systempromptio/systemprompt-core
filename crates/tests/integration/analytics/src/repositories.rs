//! Smoke integration tests covering every repository on the
//! `systemprompt_analytics` crate. The tests insert just enough data to drive
//! each public query path, asserting cheaply that the call succeeds and that
//! shape/aggregate invariants hold. They focus on covering query branches
//! (engaged_only, with_filters, attribution edges) rather than business logic.

use anyhow::Result;
use chrono::{Duration, TimeZone, Utc};
use sqlx::PgPool;
use systemprompt_analytics::models::{
    AnalyticsEventType, CreateAnalyticsEventInput, CreateEngagementEventInput, FlagReason,
};
use systemprompt_analytics::{
    AnalyticsEventsRepository, ConversationAnalyticsRepository, EngagementRepository,
    FingerprintRepository, OverviewAnalyticsRepository, RequestAnalyticsRepository,
    TrafficAnalyticsRepository,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
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

#[allow(dead_code)]
struct Fixture {
    pool: PgPool,
    db: DbPool,
    user_id: String,
    user_typed: UserId,
    context_id: String,
    tag: String,
    window_start: chrono::DateTime<Utc>,
    window_end: chrono::DateTime<Utc>,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url = fixture_database_url()?;
        let db = fixture_db_pool(&url).await?;
        let pool = db.pool_arc()?.as_ref().clone();
        let tag = Uuid::new_v4().simple().to_string();
        let user_id = format!("repo_u_{tag}");
        let context_id = format!("repo_c_{tag}");

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(&user_id)
            .bind(&user_id)
            .bind(format!("{user_id}@test.invalid"))
            .execute(&pool)
            .await?;
        let uuid = Uuid::new_v4();
        let offset_days = i64::from(u32::from_le_bytes(
            uuid.as_bytes()[0..4].try_into().unwrap(),
        ));
        let base = Utc.with_ymd_and_hms(2099, 6, 1, 0, 0, 0).unwrap();
        let window_start = base + Duration::days(offset_days % 1_000_000);
        let window_end = window_start + Duration::days(1);

        sqlx::query(
            "INSERT INTO user_contexts (context_id, user_id, name, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $4)",
        )
        .bind(&context_id)
        .bind(&user_id)
        .bind(format!("ctx-{tag}"))
        .bind(window_start + Duration::minutes(1))
        .execute(&pool)
        .await?;

        Ok(Self {
            user_typed: UserId::new(&user_id),
            pool,
            db,
            user_id,
            context_id,
            tag,
            window_start,
            window_end,
            _guard: guard,
        })
    }

    async fn insert_session(
        &self,
        session_id: &str,
        is_bot: bool,
        referrer_source: Option<&str>,
        country: Option<&str>,
        device_type: Option<&str>,
        browser: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_sessions (session_id, user_id, started_at, last_activity_at, \
             is_bot, is_behavioral_bot, is_scanner, referrer_source, country, device_type, \
             browser, user_agent, landing_page, request_count) VALUES ($1, $2, $3, $3, $4, \
             false, false, $5, $6, $7, $8, $9, '/', 5)",
        )
        .bind(session_id)
        .bind(&self.user_id)
        .bind(self.window_start + Duration::minutes(1))
        .bind(is_bot)
        .bind(referrer_source)
        .bind(country)
        .bind(device_type)
        .bind(browser)
        .bind(user_agent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn insert_ai_request(&self, model: &str, cost: i64, tokens: i64) -> Result<()> {
        let id = format!("req_{}_{}", self.tag, Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, provider, model, \
             cost_microdollars, tokens_used, status, created_at, updated_at, actor_kind, \
             actor_id, latency_ms) VALUES ($1, $2, $3, 'p', $4, $5, $6, 'completed', $7, $7, \
             'user', $3, 100)",
        )
        .bind(&id)
        .bind(&id)
        .bind(&self.user_id)
        .bind(model)
        .bind(cost)
        .bind(tokens)
        .bind(self.window_start + Duration::minutes(2))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn cleanup(&self) -> Result<()> {
        let _ = sqlx::query("DELETE FROM engagement_events WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM analytics_events WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM ai_requests WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM user_contexts WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        Ok(())
    }
}

#[tokio::test]
async fn traffic_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    fx.insert_session(
        &format!("s_{}_1", fx.tag),
        false,
        Some("google"),
        Some("US"),
        Some("desktop"),
        Some("chrome"),
        Some("Mozilla/5.0"),
    )
    .await?;
    fx.insert_session(
        &format!("s_{}_2", fx.tag),
        true,
        Some("direct"),
        Some("DE"),
        Some("mobile"),
        Some("safari"),
        Some("Googlebot/2.1"),
    )
    .await?;

    let repo = TrafficAnalyticsRepository::new(&fx.db)?;
    let sources = repo
        .get_sources(fx.window_start, fx.window_end, 50, false)
        .await?;
    let sources_engaged = repo
        .get_sources(fx.window_start, fx.window_end, 50, true)
        .await?;
    let geo = repo
        .get_geo_breakdown(fx.window_start, fx.window_end, 50, false)
        .await?;
    let geo_engaged = repo
        .get_geo_breakdown(fx.window_start, fx.window_end, 50, true)
        .await?;
    let dev = repo
        .get_device_breakdown(fx.window_start, fx.window_end, 50, false)
        .await?;
    let dev_engaged = repo
        .get_device_breakdown(fx.window_start, fx.window_end, 50, true)
        .await?;
    let bot_totals = repo
        .get_bot_totals(fx.window_start, fx.window_end, false)
        .await?;
    let bot_engaged = repo
        .get_bot_totals(fx.window_start, fx.window_end, true)
        .await?;
    let bot_breakdown = repo.get_bot_breakdown(fx.window_start, fx.window_end).await?;

    assert!(!sources.is_empty());
    let _ = sources_engaged;
    assert!(!geo.is_empty());
    let _ = geo_engaged;
    assert!(!dev.is_empty());
    let _ = dev_engaged;
    assert!(bot_totals.human >= 1);
    assert!(bot_totals.bot >= 1);
    assert!(bot_engaged.human <= bot_totals.human);
    assert!(bot_breakdown
        .iter()
        .any(|r| r.bot_type.as_deref() == Some("Google")));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn overview_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    fx.insert_ai_request("model-a", 1_000, 50).await?;
    fx.insert_session(
        &format!("s_{}_o", fx.tag),
        false,
        Some("direct"),
        Some("US"),
        Some("desktop"),
        Some("chrome"),
        Some("Mozilla/5.0"),
    )
    .await?;

    let repo = OverviewAnalyticsRepository::new(&fx.db)?;
    let conv_count = repo
        .get_conversation_count(fx.window_start, fx.window_end)
        .await?;
    let _agent = repo
        .get_agent_metrics(fx.window_start, fx.window_end)
        .await?;
    let req = repo
        .get_request_metrics(fx.window_start, fx.window_end)
        .await?;
    let _tool = repo
        .get_tool_metrics(fx.window_start, fx.window_end)
        .await?;
    let active = repo
        .get_active_session_count(fx.window_start)
        .await?;
    let total = repo
        .get_total_session_count(fx.window_start, fx.window_end)
        .await?;
    let cost = repo.get_cost(fx.window_start, fx.window_end).await?;

    assert!(conv_count >= 1);
    assert!(req.total >= 1);
    assert!(active >= 1);
    assert!(total >= 1);
    assert_eq!(cost.cost, Some(1_000));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn engagement_repository_lifecycle() -> Result<()> {
    let fx = Fixture::new().await?;
    let session_id = SessionId::new(format!("eng_s_{}", fx.tag));
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, started_at, last_activity_at) VALUES \
         ($1, $2, $3, $3)",
    )
    .bind(session_id.as_str())
    .bind(&fx.user_id)
    .bind(fx.window_start)
    .execute(&fx.pool)
    .await?;

    let repo = EngagementRepository::new(&fx.db)?;
    let input = CreateEngagementEventInput {
        page_url: "/home".into(),
        event_type: "page_exit".into(),
        time_on_page_ms: 1500,
        max_scroll_depth: 80,
        click_count: 3,
        ..Default::default()
    };
    let event_id = repo
        .create_engagement(&session_id, &fx.user_typed, None, &input)
        .await?;
    let by_id = repo.find_by_id(&event_id).await?;
    assert!(by_id.is_some());
    let by_session = repo.list_by_session(&session_id).await?;
    assert_eq!(by_session.len(), 1);
    let by_user = repo.list_by_user(&fx.user_typed, 10).await?;
    assert!(!by_user.is_empty());
    let summary = repo.get_session_engagement_summary(&session_id).await?;
    assert!(summary.is_some());
    let summary = summary.unwrap();
    assert_eq!(summary.page_count, Some(1));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn fingerprint_repository_lifecycle() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = FingerprintRepository::new(&fx.db)?;
    let fp_hash = format!("fp_{}", fx.tag);

    let rep = repo
        .upsert_fingerprint(&fp_hash, Some("1.2.3.4"), Some("ua"), Some(&fx.user_typed))
        .await?;
    assert_eq!(rep.fingerprint_hash, fp_hash);

    let rep2 = repo
        .upsert_fingerprint(&fp_hash, Some("1.2.3.4"), Some("ua"), Some(&fx.user_typed))
        .await?;
    assert!(rep2.total_session_count >= 2);

    repo.update_velocity_metrics(&fp_hash, 10, 2.5, 4).await?;
    repo.update_active_session_count(&fp_hash, 3).await?;
    repo.increment_request_count(&fp_hash).await?;
    let score_after = repo.adjust_reputation_score(&fp_hash, -10).await?;
    assert!(score_after <= 100);
    repo.flag_fingerprint(&fp_hash, FlagReason::HighRequestCount, score_after)
        .await?;
    repo.clear_flag(&fp_hash).await?;

    let by_hash = repo.get_by_hash(&fp_hash).await?;
    assert!(by_hash.is_some());
    let _active = repo.count_active_sessions(&fp_hash).await?;
    let _reuse = repo.find_reusable_session(&fp_hash).await?;
    let _analysis = repo.get_fingerprints_for_analysis().await?;
    let high = repo.get_high_risk_fingerprints(50).await?;
    assert!(high.iter().all(|r| !r.fingerprint_hash.is_empty()));

    let _ = sqlx::query("DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1")
        .bind(&fp_hash)
        .execute(&fx.pool)
        .await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn request_analytics_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    fx.insert_ai_request("m1", 100, 10).await?;
    fx.insert_ai_request("m2", 200, 20).await?;

    let repo = RequestAnalyticsRepository::new(&fx.db)?;
    let stats = repo.get_stats(fx.window_start, fx.window_end, None).await?;
    assert!(stats.total >= 2);
    let stats_filtered = repo
        .get_stats(fx.window_start, fx.window_end, Some("m1"))
        .await?;
    assert!(stats_filtered.total >= 1);
    let models = repo.list_models(fx.window_start, fx.window_end, 10).await?;
    assert!(models.iter().any(|m| m.model == "m1"));
    let trends = repo
        .get_requests_for_trends(fx.window_start, fx.window_end)
        .await?;
    assert!(trends.len() >= 2);
    let listed = repo
        .list_requests(fx.window_start, fx.window_end, 100, None)
        .await?;
    assert!(!listed.is_empty());
    let listed_filtered = repo
        .list_requests(fx.window_start, fx.window_end, 100, Some("m2"))
        .await?;
    assert!(!listed_filtered.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn conversation_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = ConversationAnalyticsRepository::new(&fx.db)?;
    let _agent = repo
        .list_agent_contexts(fx.window_start, fx.window_end, 50)
        .await?;
    let _gw = repo
        .list_gateway_sessions(fx.window_start, fx.window_end, 50)
        .await?;
    let ctx_ct = repo.get_context_count(fx.window_start, fx.window_end).await?;
    assert!(ctx_ct >= 1);
    let _tasks = repo.get_task_stats(fx.window_start, fx.window_end).await?;
    let msg = repo.get_message_count(fx.window_start, fx.window_end).await?;
    assert!(msg >= 0);
    let ctx_ts = repo
        .get_context_timestamps(fx.window_start, fx.window_end)
        .await?;
    assert!(!ctx_ts.is_empty());
    let _task_ts = repo
        .get_task_timestamps(fx.window_start, fx.window_end)
        .await?;
    let _msg_ts = repo
        .get_message_timestamps(fx.window_start, fx.window_end)
        .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn events_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AnalyticsEventsRepository::new(&fx.db)?;

    let session_id = SessionId::new(format!("ev_s_{}", fx.tag));
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, started_at, last_activity_at) VALUES \
         ($1, $2, $3, $3)",
    )
    .bind(session_id.as_str())
    .bind(&fx.user_id)
    .bind(fx.window_start)
    .execute(&fx.pool)
    .await?;

    let input = CreateAnalyticsEventInput {
        event_type: AnalyticsEventType::PageView,
        page_url: "/home".into(),
        content_id: None,
        slug: None,
        referrer: Some("https://example.com".into()),
        data: Some(serde_json::json!({"k": "v"})),
    };
    let created = repo
        .create_event(&session_id, &fx.user_typed, &input)
        .await?;
    assert_eq!(created.event_type, "page_view");

    let batch = vec![
        CreateAnalyticsEventInput {
            event_type: AnalyticsEventType::Scroll,
            page_url: "/p1".into(),
            content_id: None,
            slug: None,
            referrer: None,
            data: None,
        },
        CreateAnalyticsEventInput {
            event_type: AnalyticsEventType::LinkClick,
            page_url: "/p2".into(),
            content_id: None,
            slug: None,
            referrer: None,
            data: None,
        },
    ];
    let batch_out = repo
        .create_events_batch(&session_id, &fx.user_typed, &batch)
        .await?;
    assert_eq!(batch_out.len(), 2);

    let empty_batch = repo
        .create_events_batch(&session_id, &fx.user_typed, &[])
        .await?;
    assert!(empty_batch.is_empty());

    let count = repo
        .count_events_by_type(&session_id, &AnalyticsEventType::PageView)
        .await?;
    assert!(count >= 1);

    let by_session = repo.find_by_session(&session_id, 10).await?;
    assert!(!by_session.is_empty());

    fx.cleanup().await?;
    Ok(())
}
