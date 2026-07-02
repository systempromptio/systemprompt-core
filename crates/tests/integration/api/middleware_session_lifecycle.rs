//! Session-establishment lifecycle paths driven through `SessionMiddleware`.
//!
//! Each test builds a one-route router wrapped in the real middleware and sends
//! a request that steers `resolve_session` down a distinct branch: no token,
//! garbage token, a valid JWT whose session is still active, a valid JWT whose
//! session was revoked (refresh), a valid JWT for a missing user (anonymous
//! re-create), and a bot user-agent that short-circuits to an anonymous
//! context. The skip-tracked path is covered by
//! `session_middleware_persists_anon`.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, header};
use axum::routing::get;
use axum::{Router, middleware};
use systemprompt_api::services::middleware::SessionMiddleware;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, install_test_signing_key, mint_admin_jwt,
    seed_admin_credential, seed_user_row,
};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn ok_handler() -> &'static str {
    "ok"
}

async fn router() -> Result<(DbPool, Router)> {
    let b = ensure_test_bootstrap();
    let _ = systemprompt_models::Config::install(fixture_config(&b.database_url));
    let (db, ctx) = setup_ctx().await?;
    install_test_signing_key();
    let mw = SessionMiddleware::new(&ctx)?;
    let app = Router::new()
        .route("/page", get(ok_handler))
        .layer(middleware::from_fn(move |req, next| {
            let mw = mw.clone();
            async move { mw.handle(req, next).await }
        }));
    Ok((db, app))
}

fn get_page(headers: &[(&str, String)]) -> Request<Body> {
    let mut builder = Request::builder().uri("/page");
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    builder.body(Body::empty()).expect("request build")
}

#[tokio::test]
async fn tracked_request_without_token_mints_session_cookie() -> Result<()> {
    let (_db, app) = router().await?;
    let ua = format!(
        "Mozilla/5.0 (X11; Linux x86_64) fresh/{}",
        uuid::Uuid::new_v4()
    );
    let resp = app.oneshot(get_page(&[("user-agent", ua)])).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    assert!(
        resp.headers().get(header::SET_COOKIE).is_some(),
        "a freshly minted session issues a Set-Cookie"
    );
    Ok(())
}

#[tokio::test]
async fn tracked_request_with_garbage_token_creates_new_session() -> Result<()> {
    let (_db, app) = router().await?;
    let ua = format!("Mozilla/5.0 (Macintosh) garbage/{}", uuid::Uuid::new_v4());
    let resp = app
        .oneshot(get_page(&[
            ("user-agent", ua),
            ("authorization", "Bearer not-a-real-jwt".to_owned()),
        ]))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    assert!(
        resp.headers().get(header::SET_COOKIE).is_some(),
        "an undecodable token falls back to a fresh session"
    );
    Ok(())
}

#[tokio::test]
async fn tracked_request_with_active_session_reuses_it() -> Result<()> {
    let (db, app) = router().await?;
    let fixture = seed_admin_credential(&db, "session-reuse").await?;

    let resp = app
        .oneshot(get_page(&[
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0)".to_owned()),
            ("authorization", format!("Bearer {}", fixture.jwt.as_str())),
        ]))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    assert!(
        resp.headers().get(header::SET_COOKIE).is_none(),
        "an existing active session is reused without a new cookie"
    );
    Ok(())
}

#[tokio::test]
async fn tracked_request_with_revoked_session_refreshes() -> Result<()> {
    let (db, app) = router().await?;
    let fixture = seed_admin_credential(&db, "session-refresh").await?;

    let p = db.pool_arc()?;
    sqlx::query("UPDATE user_sessions SET revoked_at = NOW() WHERE session_id = $1")
        .bind(fixture.session_id.as_str())
        .execute(p.as_ref())
        .await?;

    let resp = app
        .oneshot(get_page(&[
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0)".to_owned()),
            ("authorization", format!("Bearer {}", fixture.jwt.as_str())),
        ]))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    assert!(
        resp.headers().get(header::SET_COOKIE).is_some(),
        "a valid JWT with a missing session refreshes and re-cookies"
    );
    Ok(())
}

#[tokio::test]
async fn tracked_request_with_unknown_user_creates_anonymous_session() -> Result<()> {
    let (_db, app) = router().await?;
    let ghost = UserId::new(format!("ghost-{}", uuid::Uuid::new_v4()));
    let token = mint_admin_jwt(&ghost, "ghost@example.invalid", "test-admin");

    let resp = app
        .oneshot(get_page(&[
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0)".to_owned()),
            ("authorization", format!("Bearer {}", token.as_str())),
        ]))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn tracked_request_with_valid_user_no_session_row_refreshes() -> Result<()> {
    let (db, app) = router().await?;
    let user_id = UserId::new(format!("live-user-{}", uuid::Uuid::new_v4()));
    seed_user_row(&db, &user_id, "live@example.invalid").await?;
    let token = mint_admin_jwt(&user_id, "live@example.invalid", "test-admin");

    let resp = app
        .oneshot(get_page(&[
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0)".to_owned()),
            ("authorization", format!("Bearer {}", token.as_str())),
        ]))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn bot_user_agent_yields_anonymous_context() -> Result<()> {
    let (_db, app) = router().await?;
    let resp = app
        .oneshot(get_page(&[("user-agent", "Googlebot/2.1".to_owned())]))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    assert!(
        resp.headers().get(header::SET_COOKIE).is_none(),
        "bots never get a tracked session cookie"
    );
    Ok(())
}
