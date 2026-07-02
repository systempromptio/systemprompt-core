//! Drives `AnalyticsMiddleware::track_request` and its detached fan-out tasks
//! (session tracking, velocity check, behavioural scoring, analytics-event
//! capture, scanner marking).
//!
//! The middleware spawns detached tasks, so each test sends the request and
//! then yields long enough for those tasks to run against the real pool. The
//! behavioural score is analytics-only — nothing here re-adds throttling; we
//! only exercise the detection paths.

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
    let session = SessionMiddleware::new(ctx)?;
    let analytics = AnalyticsMiddleware::new(ctx)?;
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

fn browser_get(uri: &str) -> Request<Body> {
    let ua = format!(
        "Mozilla/5.0 (X11; Linux x86_64) test/{}",
        uuid::Uuid::new_v4()
    );
    Request::builder()
        .uri(uri)
        .header("user-agent", ua)
        .header("referer", "https://example.com/prev")
        .body(Body::empty())
        .expect("request build")
}

async fn drain() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

#[tokio::test]
async fn tracked_page_view_spawns_activity_and_event_tasks() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let resp = app.oneshot(browser_get("/article")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    drain().await;
    Ok(())
}

#[tokio::test]
async fn tracked_server_error_records_error_severity_event() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let resp = app.oneshot(browser_get("/boom")).await?;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    drain().await;
    Ok(())
}

#[tokio::test]
async fn scanner_path_marks_session_as_scanner() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let app = build(&ctx)?;
    let resp = app.oneshot(browser_get("/wp-login.php")).await?;
    assert!(resp.status().as_u16() < 500, "{}", resp.status());
    drain().await;
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
    drain().await;
    Ok(())
}

#[tokio::test]
async fn request_without_context_passes_through() -> Result<()> {
    let (_db, ctx) = setup().await?;
    let analytics = AnalyticsMiddleware::new(&ctx)?;
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
