//! Coverage for the dedicated homepage handler and the session-bootstrap
//! helper.
//!
//! `serve_homepage` is driven directly against a `tempfile` web `dist` for the
//! present / 304 / read-error-free / missing branches, and `ensure_session`
//! is exercised on its anonymous-fallback path (no bearer token) against the
//! fixture app context.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use systemprompt_api::services::static_content::config::StaticContentMatcher;
use systemprompt_api::services::static_content::serve_homepage;
use systemprompt_api::services::static_content::session::ensure_session;
use systemprompt_api::services::static_content::static_files::{StaticContentState, compute_etag};
use systemprompt_identifiers::JwtToken;
use systemprompt_models::RouteClassifier;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context};

async fn state_with_dist() -> anyhow::Result<StaticContentState> {
    let ctx = test_app_context(&ensure_test_bootstrap().database_url).await;
    std::fs::create_dir_all(ctx.app_paths().web().dist())?;
    let state = StaticContentState {
        ctx,
        matcher: Arc::new(StaticContentMatcher::empty()),
        route_classifier: Arc::new(RouteClassifier::new(None)),
    };
    Ok(state)
}

fn dist(state: &StaticContentState) -> std::path::PathBuf {
    state.ctx.app_paths().web().dist().to_path_buf()
}

async fn homepage(state: &StaticContentState, headers: HeaderMap) -> (StatusCode, HeaderMap) {
    let resp = serve_homepage(State(state.clone()), headers)
        .await
        .into_response();
    (resp.status(), resp.headers().clone())
}

#[tokio::test]
async fn serve_homepage_serves_index() -> anyhow::Result<()> {
    let state = state_with_dist().await?;
    std::fs::write(dist(&state).join("index.html"), "<h1>home</h1>")?;
    let (status, hdrs) = homepage(&state, HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        hdrs.get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert!(hdrs.contains_key(header::ETAG));
    Ok(())
}

#[tokio::test]
async fn serve_homepage_returns_304_on_matching_etag() -> anyhow::Result<()> {
    let state = state_with_dist().await?;
    let content = b"<h1>home</h1>";
    std::fs::write(dist(&state).join("index.html"), content)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::IF_NONE_MATCH,
        compute_etag(content).parse().unwrap(),
    );
    let (status, _hdrs) = homepage(&state, headers).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    Ok(())
}

#[tokio::test]
async fn serve_homepage_missing_index_is_404() -> anyhow::Result<()> {
    let state = state_with_dist().await?;
    let (status, _hdrs) = homepage(&state, HeaderMap::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn ensure_session_mints_anonymous_session_without_token() -> anyhow::Result<()> {
    ensure_test_bootstrap();
    let state = state_with_dist().await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        format!("cov-anon/{}", uuid::Uuid::new_v4())
            .parse()
            .unwrap(),
    );
    let info = ensure_session(&headers, None, None, &state.ctx).await?;
    assert!(info.jwt_token.is_some(), "a bearer token is issued");
    assert!(!info.session_id.as_str().is_empty());
    assert!(!info.user_id.as_str().is_empty());
    Ok(())
}

#[tokio::test]
async fn ensure_session_reuses_a_valid_browser_token_without_creating_another_session()
-> anyhow::Result<()> {
    ensure_test_bootstrap();
    let state = state_with_dist().await?;
    let mut first_headers = HeaderMap::new();
    first_headers.insert(
        "user-agent",
        format!("session-reuse/{}", uuid::Uuid::new_v4()).parse()?,
    );
    let first = ensure_session(&first_headers, None, None, &state.ctx).await?;
    let raw = state.ctx.db_pool().pool();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_sessions WHERE user_id = $1")
        .bind(first.user_id.as_str())
        .fetch_one(raw.as_ref())
        .await?;

    let token = first
        .jwt_token
        .as_ref()
        .map(JwtToken::as_str)
        .expect("anonymous session token");
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("access_token={token}")
            .parse()
            .expect("cookie header"),
    );
    let reused = ensure_session(&headers, None, None, &state.ctx).await?;

    assert_eq!(reused.session_id, first.session_id);
    assert_eq!(reused.user_id, first.user_id);
    assert!(!reused.is_new);
    assert_eq!(reused.jwt_token.as_ref().map(JwtToken::as_str), Some(token));
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_sessions WHERE user_id = $1")
        .bind(first.user_id.as_str())
        .fetch_one(raw.as_ref())
        .await?;
    assert_eq!(after, before, "token reuse must not mint another session");
    Ok(())
}
