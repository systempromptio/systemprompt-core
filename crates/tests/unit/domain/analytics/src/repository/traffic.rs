//! DB-backed tests for `TrafficAnalyticsRepository::get_pages` and
//! `get_navigation`: landing-page grouping with referrer/path-prefix filters
//! and bot exclusion, and link-click transition grouping with the
//! internal-only default. Each test scopes its rows with a unique path prefix
//! so concurrent suites sharing the database cannot pollute the counts.

use chrono::{Duration, Utc};
use systemprompt_analytics::{
    AnalyticsEventType, AnalyticsEventsRepository, CreateAnalyticsEventInput, CreateSessionParams,
    LinkClickEventData, NavigationQuery, PageQuery, SessionRepository, TrafficAnalyticsRepository,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

struct SeededSession<'a> {
    landing_page: &'a str,
    referrer_source: Option<&'a str>,
    request_count: i32,
    is_bot: bool,
    is_behavioral_bot: bool,
}

async fn seed_session(pool: &DbPool, spec: &SeededSession<'_>) -> SessionId {
    let sid = SessionId::new(format!("sess-traffic-{}", Uuid::new_v4()));
    let repo = SessionRepository::new(pool).expect("session repo");
    let params = CreateSessionParams {
        session_id: &sid,
        user_id: None,
        session_source: SessionSource::Web,
        fingerprint_hash: Some("fp"),
        ip_address: None,
        user_agent: None,
        device_type: None,
        browser: None,
        os: None,
        country: None,
        region: None,
        city: None,
        preferred_locale: None,
        referrer_source: spec.referrer_source,
        referrer_url: None,
        landing_page: Some(spec.landing_page),
        entry_url: None,
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        utm_content: None,
        utm_term: None,
        is_bot: spec.is_bot,
        is_ai_crawler: false,
        expires_at: Utc::now() + Duration::hours(1),
    };
    repo.create_session(&params).await.expect("seed session");

    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "UPDATE user_sessions SET request_count = $2, is_behavioral_bot = $3 WHERE session_id = $1",
    )
    .bind(sid.as_str())
    .bind(spec.request_count)
    .bind(spec.is_behavioral_bot)
    .execute(p.as_ref())
    .await
    .expect("update session flags");
    sid
}

async fn cleanup_sessions(pool: &DbPool, prefix: &str) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "DELETE FROM analytics_events WHERE session_id IN \
         (SELECT session_id FROM user_sessions WHERE landing_page LIKE $1 || '%')",
    )
    .bind(prefix)
    .execute(p.as_ref())
    .await
    .ok();
    sqlx::query("DELETE FROM user_sessions WHERE landing_page LIKE $1 || '%'")
        .bind(prefix)
        .execute(p.as_ref())
        .await
        .ok();
}

fn window() -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>) {
    (
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::hours(1),
    )
}

#[tokio::test]
async fn get_pages_groups_by_landing_page_and_referrer_with_filters() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = TrafficAnalyticsRepository::new(&pool).expect("repo");

    let prefix = format!("/tp-{}", Uuid::new_v4());
    let guide = format!("{prefix}/guides/a");
    let tool = format!("{prefix}/tools/b");

    for _ in 0..2 {
        seed_session(
            &pool,
            &SeededSession {
                landing_page: &guide,
                referrer_source: Some("google"),
                request_count: 1,
                is_bot: false,
                is_behavioral_bot: false,
            },
        )
        .await;
    }
    seed_session(
        &pool,
        &SeededSession {
            landing_page: &tool,
            referrer_source: None,
            request_count: 1,
            is_bot: false,
            is_behavioral_bot: false,
        },
    )
    .await;
    seed_session(
        &pool,
        &SeededSession {
            landing_page: &guide,
            referrer_source: Some("google"),
            request_count: 1,
            is_bot: true,
            is_behavioral_bot: false,
        },
    )
    .await;
    seed_session(
        &pool,
        &SeededSession {
            landing_page: &guide,
            referrer_source: Some("google"),
            request_count: 1,
            is_bot: false,
            is_behavioral_bot: true,
        },
    )
    .await;

    let (start, end) = window();
    let rows = repo
        .get_pages(PageQuery {
            start,
            end,
            limit: 20,
            engaged_only: true,
            referrer: None,
            path_prefix: Some(&prefix),
        })
        .await
        .expect("get_pages");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].page.as_deref(), Some(guide.as_str()));
    assert_eq!(rows[0].source.as_deref(), Some("google"));
    assert_eq!(rows[0].count, 2);
    assert_eq!(rows[1].page.as_deref(), Some(tool.as_str()));
    assert_eq!(rows[1].source.as_deref(), Some("direct"));
    assert_eq!(rows[1].count, 1);

    let google_only = repo
        .get_pages(PageQuery {
            start,
            end,
            limit: 20,
            engaged_only: true,
            referrer: Some("google"),
            path_prefix: Some(&prefix),
        })
        .await
        .expect("get_pages referrer");
    assert_eq!(google_only.len(), 1);
    assert_eq!(google_only[0].page.as_deref(), Some(guide.as_str()));

    let direct_only = repo
        .get_pages(PageQuery {
            start,
            end,
            limit: 20,
            engaged_only: true,
            referrer: Some("direct"),
            path_prefix: Some(&prefix),
        })
        .await
        .expect("get_pages direct");
    assert_eq!(direct_only.len(), 1);
    assert_eq!(direct_only[0].page.as_deref(), Some(tool.as_str()));

    let tools_only = repo
        .get_pages(PageQuery {
            start,
            end,
            limit: 20,
            engaged_only: true,
            referrer: None,
            path_prefix: Some(&format!("{prefix}/tools")),
        })
        .await
        .expect("get_pages path prefix");
    assert_eq!(tools_only.len(), 1);
    assert_eq!(tools_only[0].page.as_deref(), Some(tool.as_str()));

    cleanup_sessions(&pool, &prefix).await;
}

#[tokio::test]
async fn get_pages_engaged_only_excludes_zero_request_sessions() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = TrafficAnalyticsRepository::new(&pool).expect("repo");

    let prefix = format!("/tp-{}", Uuid::new_v4());
    let page = format!("{prefix}/landing");
    seed_session(
        &pool,
        &SeededSession {
            landing_page: &page,
            referrer_source: Some("news"),
            request_count: 0,
            is_bot: false,
            is_behavioral_bot: false,
        },
    )
    .await;

    let (start, end) = window();
    let engaged = repo
        .get_pages(PageQuery {
            start,
            end,
            limit: 20,
            engaged_only: true,
            referrer: None,
            path_prefix: Some(&prefix),
        })
        .await
        .expect("engaged");
    assert!(engaged.is_empty());

    let all = repo
        .get_pages(PageQuery {
            start,
            end,
            limit: 20,
            engaged_only: false,
            referrer: None,
            path_prefix: Some(&prefix),
        })
        .await
        .expect("all");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].count, 1);

    cleanup_sessions(&pool, &prefix).await;
}

async fn seed_link_click(
    events: &AnalyticsEventsRepository,
    sid: &SessionId,
    from: &str,
    to: &str,
    is_external: bool,
) {
    let data = serde_json::to_value(LinkClickEventData {
        target_url: Some(to.to_owned()),
        link_text: None,
        link_position: None,
        is_external: Some(is_external),
    })
    .expect("event data");
    let input = CreateAnalyticsEventInput {
        event_type: AnalyticsEventType::LinkClick,
        page_url: from.to_owned(),
        content_id: None,
        slug: None,
        referrer: None,
        data: Some(data),
    };
    events
        .create_event(sid, &UserId::new("anon".to_owned()), &input)
        .await
        .expect("seed link click");
}

#[tokio::test]
async fn get_navigation_groups_internal_link_clicks_by_transition() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = TrafficAnalyticsRepository::new(&pool).expect("repo");
    let events = AnalyticsEventsRepository::new(&pool).expect("events repo");

    let prefix = format!("/tn-{}", Uuid::new_v4());
    let sid = seed_session(
        &pool,
        &SeededSession {
            landing_page: &format!("{prefix}/guides/a"),
            referrer_source: None,
            request_count: 1,
            is_bot: false,
            is_behavioral_bot: false,
        },
    )
    .await;

    let guide = format!("{prefix}/guides/a");
    let tool = format!("{prefix}/tools/b");
    let other = format!("{prefix}/docs/c");
    for _ in 0..2 {
        seed_link_click(&events, &sid, &guide, &tool, false).await;
    }
    seed_link_click(&events, &sid, &guide, &other, false).await;
    seed_link_click(&events, &sid, &guide, "https://example.com/ext", true).await;

    let (start, end) = window();
    let rows = repo
        .get_navigation(NavigationQuery {
            start,
            end,
            limit: 20,
            path_prefix: Some(&prefix),
            internal_only: true,
        })
        .await
        .expect("get_navigation");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].from_path.as_deref(), Some(guide.as_str()));
    assert_eq!(rows[0].to_path.as_deref(), Some(tool.as_str()));
    assert_eq!(rows[0].count, 2);
    assert_eq!(rows[1].to_path.as_deref(), Some(other.as_str()));
    assert_eq!(rows[1].count, 1);

    let tools_only = repo
        .get_navigation(NavigationQuery {
            start,
            end,
            limit: 20,
            path_prefix: Some(&format!("{prefix}/tools")),
            internal_only: true,
        })
        .await
        .expect("get_navigation prefix");
    assert_eq!(tools_only.len(), 1);
    assert_eq!(tools_only[0].to_path.as_deref(), Some(tool.as_str()));

    cleanup_sessions(&pool, &prefix).await;
}

#[tokio::test]
async fn get_navigation_include_external_returns_external_clicks() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = TrafficAnalyticsRepository::new(&pool).expect("repo");
    let events = AnalyticsEventsRepository::new(&pool).expect("events repo");

    let prefix = format!("/tn-{}", Uuid::new_v4());
    let sid = seed_session(
        &pool,
        &SeededSession {
            landing_page: &format!("{prefix}/guides/a"),
            referrer_source: None,
            request_count: 1,
            is_bot: false,
            is_behavioral_bot: false,
        },
    )
    .await;

    let guide = format!("{prefix}/guides/a");
    let external = format!("https:{prefix}/off-site");
    seed_link_click(&events, &sid, &guide, &external, true).await;

    let (start, end) = window();
    let internal = repo
        .get_navigation(NavigationQuery {
            start,
            end,
            limit: 20,
            path_prefix: Some(&format!("https:{prefix}")),
            internal_only: true,
        })
        .await
        .expect("internal");
    assert!(internal.is_empty());

    let all = repo
        .get_navigation(NavigationQuery {
            start,
            end,
            limit: 20,
            path_prefix: Some(&format!("https:{prefix}")),
            internal_only: false,
        })
        .await
        .expect("all");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].to_path.as_deref(), Some(external.as_str()));

    cleanup_sessions(&pool, &prefix).await;
}
