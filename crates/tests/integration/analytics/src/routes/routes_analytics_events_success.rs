//! Analytics event ingestion success and fan-out paths.
//!
//! The lenient `routes_analytics_more` suite tolerates a 500 because it never
//! seeds the `user_sessions` row the `analytics_events` FK requires, so the
//! write fails and the `CREATED` / `page_exit` fan-out branches never execute.
//! Here the owning user and session are seeded first, so `create_event` and
//! `create_events_batch` succeed and drive the engagement fan-out.

use axum::Extension;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{seed_user_row, seed_user_session};
use tower::ServiceExt;

use super::common::{analytics_router, json_post, seed_content, setup_ctx};

async fn seed_session_ctx(db: &DbPool) -> anyhow::Result<RequestContext> {
    let user = UserId::new(format!("an-succ-{}", uuid::Uuid::new_v4()));
    let session = SessionId::generate();
    seed_user_row(db, &user, &format!("{}@example.com", user.as_str())).await?;
    seed_user_session(db, &user, &session).await?;
    Ok(RequestContext::new(
        session,
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(user),
    ))
}

#[tokio::test]
async fn record_event_created_with_seeded_session() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let req_ctx = seed_session_ctx(&db).await?;
    let (_slug, page_url) = seed_content(&db).await?;
    let app = analytics_router(&ctx).layer(Extension(req_ctx));

    let body = serde_json::json!({ "event_type": "page_view", "page_url": page_url });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn page_exit_fanout_created_with_seeded_session() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let req_ctx = seed_session_ctx(&db).await?;
    let (_slug, page_url) = seed_content(&db).await?;
    let app = analytics_router(&ctx).layer(Extension(req_ctx));

    let body = serde_json::json!({
        "event_type": "page_exit",
        "page_url": page_url,
        "data": {
            "time_on_page_ms": 5000,
            "max_scroll_depth": 90,
            "click_count": 4,
            "is_rage_click": true,
            "is_dead_click": false,
            "reading_pattern": "scan",
            "scroll_velocity_avg": 1.5,
            "scroll_direction_changes": 3,
            "mouse_move_distance_px": 1200,
            "keyboard_events": 2,
            "copy_events": 1,
            "focus_time_ms": 4000,
            "blur_count": 1,
            "tab_switches": 0,
            "visible_time_ms": 4500,
            "hidden_time_ms": 500,
            "time_to_first_interaction_ms": 300,
            "time_to_first_scroll_ms": 400,
        }
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn page_exit_with_zero_time_on_page_skips_fanout() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let req_ctx = seed_session_ctx(&db).await?;
    let app = analytics_router(&ctx).layer(Extension(req_ctx));

    let body = serde_json::json!({
        "event_type": "page_exit",
        "page_url": "https://example.com/no-time",
        "data": { "max_scroll_depth": 10 }
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn page_exit_without_data_skips_fanout() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let req_ctx = seed_session_ctx(&db).await?;
    let app = analytics_router(&ctx).layer(Extension(req_ctx));

    let body = serde_json::json!({
        "event_type": "page_exit",
        "page_url": "https://example.com/no-data",
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn batch_created_with_seeded_session_and_fanout() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let req_ctx = seed_session_ctx(&db).await?;
    let (_slug, page_url) = seed_content(&db).await?;
    let app = analytics_router(&ctx).layer(Extension(req_ctx));

    let body = serde_json::json!({
        "events": [
            { "event_type": "page_view", "page_url": page_url },
            {
                "event_type": "page_exit",
                "page_url": page_url,
                "data": { "time_on_page_ms": 2000, "max_scroll_depth": 50, "click_count": 3 }
            }
        ]
    });
    let resp = app.oneshot(json_post("/events/batch", body)).await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}
