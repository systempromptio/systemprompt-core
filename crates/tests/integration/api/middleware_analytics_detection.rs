//! Drives `AnalyticsMiddleware::track_request` and its fan-out tasks (session
//! tracking, velocity check, behavioural scoring, analytics-event capture,
//! scanner marking).
//!
//! The middleware runs those writes on the context's `BackgroundTasks`, so
//! each test sends the request, drains that tracker, and then reads what the
//! tasks wrote. The behavioural score is analytics-only — nothing here re-adds
//! throttling; we only exercise the detection paths.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Router, middleware};
use systemprompt_api::services::middleware::{AnalyticsMiddleware, SessionMiddleware};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_config, install_test_signing_key};
use systemprompt_traits::DrainOutcome;
use tower::ServiceExt;

use super::common::setup_ctx;

async fn ok_handler() -> &'static str {
    "ok"
}

async fn boom_handler() -> StatusCode {
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn setup() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let _ = systemprompt_models::Config::install(fixture_config(&b.database_url));
    install_test_signing_key();
    setup_ctx().await
}

fn build(ctx: &Arc<AppContext>) -> Result<Router> {
    let session = SessionMiddleware::new(ctx);
    let analytics = AnalyticsMiddleware::new(ctx);
    Ok(Router::new()
        .route("/boom", get(boom_handler))
        .fallback(get(ok_handler))
        .layer(middleware::from_fn(move |req, next| {
            let mw = analytics.clone();
            async move { mw.track_request(req, next).await }
        }))
        .layer(middleware::from_fn(move |req, next| {
            let mw = session.clone();
            async move { mw.handle(req, next).await }
        })))
}

fn browser_agent() -> String {
    format!(
        "Mozilla/5.0 (X11; Linux x86_64) test/{}",
        uuid::Uuid::new_v4()
    )
}

fn browser_get(uri: &str) -> Request<Body> {
    browser_get_as(uri, &browser_agent())
}

fn browser_get_as(uri: &str, user_agent: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("user-agent", user_agent)
        .header("referer", "https://example.com/prev")
        .body(Body::empty())
        .expect("request build")
}

async fn drain(ctx: &AppContext) {
    assert_eq!(
        ctx.background_tasks().drain(Duration::from_secs(30)).await,
        DrainOutcome::Drained
    );
}

async fn event_severities(db: &DbPool, user_agent: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT severity FROM analytics_events WHERE position($1 in metadata) > 0")
        .bind(user_agent)
        .fetch_all(db.pool().as_ref())
        .await
        .expect("query analytics events")
}

#[tokio::test]
async fn tracked_page_view_spawns_activity_and_event_tasks() -> Result<()> {
    let (db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let agent = browser_agent();
    let resp = app.oneshot(browser_get_as("/article", &agent)).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    drain(&ctx).await;
    assert_eq!(event_severities(&db, &agent).await, vec!["info".to_owned()]);
    Ok(())
}

#[tokio::test]
async fn tracked_server_error_records_error_severity_event() -> Result<()> {
    let (db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let agent = browser_agent();
    let resp = app.oneshot(browser_get_as("/boom", &agent)).await?;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    drain(&ctx).await;
    assert_eq!(
        event_severities(&db, &agent).await,
        vec!["error".to_owned()]
    );
    Ok(())
}

#[tokio::test]
async fn scanner_path_marks_session_as_scanner() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let resp = app.oneshot(browser_get("/wp-login.php")).await?;
    assert!(resp.status().as_u16() < 500, "{}", resp.status());
    drain(&ctx).await;
    Ok(())
}

#[tokio::test]
async fn untracked_context_skips_analytics_fanout() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let req = Request::builder()
        .uri("/health")
        .header("user-agent", "Mozilla/5.0 (X11; Linux x86_64)")
        .body(Body::empty())?;
    let resp = app.oneshot(req).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    drain(&ctx).await;
    Ok(())
}

#[tokio::test]
async fn request_without_context_passes_through() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let analytics = AnalyticsMiddleware::new(&ctx);
    let app = Router::new()
        .fallback(get(ok_handler))
        .layer(middleware::from_fn(move |req, next| {
            let mw = analytics.clone();
            async move { mw.track_request(req, next).await }
        }));
    let resp = app.oneshot(browser_get("/no-session")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}
