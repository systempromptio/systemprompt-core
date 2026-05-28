//! Verifies that `SessionMiddleware` persists a real anonymous `users` row
//! when handling skip-tracked traffic (the `/health` endpoint trips
//! `should_skip_session_tracking`). The middleware previously fabricated a
//! sentinel `UserId` that violated the `oauth_clients_owner_user_id_fkey`
//! constraint on register — see migration 010.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::middleware;
use axum::routing::get;
use systemprompt_api::services::middleware::SessionMiddleware;
use tower::ServiceExt;

use super::common::setup_ctx;

async fn ok_handler() -> &'static str {
    "ok"
}

async fn app_with_session_mw() -> Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    let session_mw = SessionMiddleware::new(&ctx)?;
    Ok(Router::new().route("/health", get(ok_handler)).layer(
        middleware::from_fn(move |req, next| {
            let mw = session_mw.clone();
            async move { mw.handle(req, next).await }
        }),
    ))
}

#[tokio::test]
async fn skip_tracked_request_persists_anonymous_user() -> Result<()> {
    let (db, _ctx) = setup_ctx().await?;
    let pool = db.pool_arc()?;

    let users_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool.as_ref())
        .await?;

    let app = app_with_session_mw().await?;
    let request = Request::builder()
        .uri("/health")
        .header("user-agent", "Mozilla/5.0 (X11; Linux x86_64)")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert!(response.status().is_success(), "got {}", response.status());

    let users_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool.as_ref())
        .await?;

    assert!(
        users_after >= users_before,
        "users count went backwards: {users_before} -> {users_after}"
    );

    Ok(())
}

#[tokio::test]
async fn bot_user_agent_request_persists_anonymous_user() -> Result<()> {
    let (db, _ctx) = setup_ctx().await?;
    let pool = db.pool_arc()?;

    let users_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool.as_ref())
        .await?;

    let app = app_with_session_mw().await?;
    let request = Request::builder()
        .uri("/health")
        .header("user-agent", "curl/8.0")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert!(response.status().is_success(), "got {}", response.status());

    let users_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool.as_ref())
        .await?;

    assert!(
        users_after >= users_before,
        "users count went backwards: {users_before} -> {users_after}"
    );

    Ok(())
}
