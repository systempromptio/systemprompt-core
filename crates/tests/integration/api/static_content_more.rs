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
use systemprompt_files::FilesConfig;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::RouteClassifier;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with, fixture_config, fixture_db_pool,
    install_test_signing_key,
};
use tempfile::TempDir;

async fn state_with_dist() -> anyhow::Result<(TempDir, StaticContentState)> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let tmp = TempDir::new()?;
    let web = tmp.path().join("web");
    std::fs::create_dir_all(web.join("dist"))?;
    let paths = PathsConfig {
        system: tmp.path().to_string_lossy().into_owned(),
        services: "/tmp".to_owned(),
        bin: "/tmp".to_owned(),
        web_path: Some(web.to_string_lossy().into_owned()),
        storage: Some("/tmp".to_owned()),
        geoip_database: None,
    };
    let ctx = fixture_app_context_with(&pool, &b.database_url, paths, Arc::new(AllowAllFilter))?;
    let _ = FilesConfig::init(ctx.app_paths());
    let state = StaticContentState {
        ctx,
        matcher: Arc::new(StaticContentMatcher::empty()),
        route_classifier: Arc::new(RouteClassifier::new(None)),
    };
    Ok((tmp, state))
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
    let (_tmp, state) = state_with_dist().await?;
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
    let (_tmp, state) = state_with_dist().await?;
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
    let (_tmp, state) = state_with_dist().await?;
    let (status, _hdrs) = homepage(&state, HeaderMap::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn ensure_session_mints_anonymous_session_without_token() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    let _ = systemprompt_models::Config::install(fixture_config(&b.database_url));
    install_test_signing_key();
    let (_tmp, state) = state_with_dist().await?;

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
