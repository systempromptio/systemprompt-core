use chrono::{Duration, Utc};
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_traits::SessionStore;
use systemprompt_traits::session_store::CreateSessionParams;
use systemprompt_users::SessionRepository;

fn params<'a>(session_id: &'a SessionId, fingerprint: &'a str) -> CreateSessionParams<'a> {
    CreateSessionParams {
        session_id,
        user_id: None,
        session_source: SessionSource::Web,
        fingerprint_hash: Some(fingerprint),
        ip_address: Some("198.51.100.42"),
        user_agent: Some("session-store-lifecycle"),
        device_type: Some("desktop"),
        browser: Some("fixture"),
        os: Some("linux"),
        country: None,
        region: None,
        city: None,
        preferred_locale: Some("en"),
        referrer_source: None,
        referrer_url: None,
        landing_page: Some("/start"),
        entry_url: Some("/entry"),
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        utm_content: None,
        utm_term: None,
        is_bot: false,
        is_ai_crawler: false,
        expires_at: Utc::now() + Duration::hours(1),
    }
}

async fn isolated(
    prefix: &str,
) -> (
    DisposableDb,
    systemprompt_database::DbPool,
    SessionRepository,
) {
    let database = DisposableDb::installed(prefix)
        .await
        .expect("installed disposable DB");
    let pool = database.pool().await.expect("disposable pool");
    let repository = SessionRepository::new(&pool).expect("session repository");
    (database, pool, repository)
}

async fn cleanup(database: DisposableDb, pool: systemprompt_database::DbPool) {
    pool.write_pool_arc().expect("write pool").close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn trait_object_session_lifecycle_persists_usage_behavior_and_terminal_state() {
    let (database, pool, repository) = isolated("users_store_lifecycle").await;
    let store: &dyn SessionStore = &repository;
    let session_id = SessionId::new(format!("store-{}", uuid::Uuid::new_v4()));
    let fingerprint = format!("store-fp-{}", uuid::Uuid::new_v4());

    store
        .insert_session(&params(&session_id, &fingerprint))
        .await
        .expect("insert session");
    store
        .increment_request_count(&session_id)
        .await
        .expect("increment request");
    store
        .increment_ai_usage(&session_id, 90, 4_500)
        .await
        .expect("increment AI usage");
    store
        .update_behavioral_detection(&session_id, 87, true, Some("automated navigation"))
        .await
        .expect("persist behavioral verdict");
    assert_eq!(
        store
            .set_session_geo(&session_id, Some("ES"), Some("MD"), Some("Madrid"))
            .await
            .expect("persist geo"),
        1
    );

    let snapshot = store
        .find_by_id(&session_id)
        .await
        .expect("find session")
        .expect("session exists");
    assert_eq!(
        snapshot.fingerprint_hash.as_deref(),
        Some(fingerprint.as_str())
    );
    assert_eq!(snapshot.request_count, Some(1));
    assert_eq!(snapshot.ai_request_count, Some(1));
    let (tokens, cost_microdollars): (i32, i64) = sqlx::query_as(
        "SELECT total_tokens_used, total_ai_cost_microdollars FROM user_sessions WHERE session_id = $1",
    )
    .bind(session_id.as_str())
    .fetch_one(pool.write_pool_arc().expect("write pool").as_ref())
    .await
    .expect("persisted AI usage totals");
    assert_eq!(tokens, 90);
    assert_eq!(cost_microdollars, 4_500);
    assert_eq!(snapshot.country.as_deref(), Some("ES"));
    assert_eq!(snapshot.city.as_deref(), Some("Madrid"));
    assert_eq!(snapshot.is_behavioral_bot, Some(true));
    assert_eq!(
        snapshot.behavioral_bot_reason.as_deref(),
        Some("automated navigation")
    );

    let behavioral = store
        .get_session_for_behavioral_analysis(&session_id)
        .await
        .expect("behavioral lookup")
        .expect("behavioral row");
    assert_eq!(behavioral.request_count, Some(1));
    assert_eq!(behavioral.landing_page.as_deref(), Some("/start"));
    assert_eq!(
        store
            .count_sessions_by_fingerprint(&fingerprint, 1)
            .await
            .expect("count fingerprint"),
        1
    );
    assert_eq!(
        store
            .count_unique_ips_by_fingerprint(&fingerprint, 1)
            .await
            .expect("count IPs"),
        1
    );
    assert_eq!(
        store
            .get_session_starts_by_fingerprint(&fingerprint, 1)
            .await
            .expect("session starts")
            .len(),
        1
    );

    store.end_session(&session_id).await.expect("end session");
    assert!(
        store
            .find_active_by_id(&session_id)
            .await
            .expect("authentication lookup")
            .is_some(),
        "ending analytics duration does not revoke an unexpired authentication session"
    );
    assert!(
        store
            .find_by_id(&session_id)
            .await
            .expect("retained lookup")
            .expect("ended row retained")
            .ended_at
            .is_some()
    );
    cleanup(database, pool).await;
}

#[tokio::test]
async fn trait_object_scanner_and_threshold_transitions_are_observable() {
    let (database, pool, repository) = isolated("users_store_threshold").await;
    let store: &dyn SessionStore = &repository;
    let session_id = SessionId::new(format!("threshold-{}", uuid::Uuid::new_v4()));
    let fingerprint = format!("threshold-fp-{}", uuid::Uuid::new_v4());
    store
        .insert_session(&params(&session_id, &fingerprint))
        .await
        .expect("insert session");
    store
        .increment_request_count(&session_id)
        .await
        .expect("first request");
    assert!(
        !store
            .check_and_mark_behavioral_bot(&session_id, 1)
            .await
            .expect("threshold not yet exceeded")
    );
    store
        .increment_request_count(&session_id)
        .await
        .expect("second request crosses threshold");
    assert!(
        store
            .check_and_mark_behavioral_bot(&session_id, 1)
            .await
            .expect("threshold transition")
    );
    store
        .mark_as_scanner(&session_id)
        .await
        .expect("mark scanner");
    store
        .mark_converted(&session_id)
        .await
        .expect("mark converted");
    let converted: bool = sqlx::query_scalar(
        "SELECT converted_at IS NOT NULL FROM user_sessions WHERE session_id = $1",
    )
    .bind(session_id.as_str())
    .fetch_one(pool.write_pool_arc().expect("write pool").as_ref())
    .await
    .expect("persisted conversion timestamp");
    assert!(converted);

    let snapshot = store
        .find_by_id(&session_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(snapshot.is_scanner, Some(true));
    assert_eq!(snapshot.is_behavioral_bot, Some(true));
    assert_eq!(
        snapshot.behavioral_bot_reason.as_deref(),
        Some("request_count_exceeded")
    );
    assert_eq!(
        store
            .fingerprint_session_ids(&fingerprint, 1)
            .await
            .expect("fingerprint IDs"),
        vec![session_id]
    );
    cleanup(database, pool).await;
}

#[tokio::test]
async fn trait_object_maps_read_write_behavioral_and_geo_pool_failures() {
    let pool = systemprompt_test_fixtures::closed_db_pool().await;
    let repository = SessionRepository::new(&pool).expect("repository retains closed pool");
    let store: &dyn SessionStore = &repository;
    let session_id = SessionId::new("closed-store-session");

    for error in [
        store.find_by_id(&session_id).await.expect_err("read fails"),
        store
            .increment_request_count(&session_id)
            .await
            .expect_err("write fails"),
        store
            .get_session_for_behavioral_analysis(&session_id)
            .await
            .expect_err("behavioral read fails"),
        store
            .set_session_geo(&session_id, Some("ES"), None, None)
            .await
            .expect_err("geo write fails"),
    ] {
        assert!(matches!(
            error,
            systemprompt_traits::AnalyticsProviderError::Internal(_)
        ));
    }
}
