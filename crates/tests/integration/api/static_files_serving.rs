//! Coverage for static and prerendered content serving.
//!
//! Drives [`serve_static_content`] over a `tempfile` web `dist` directory,
//! exercising the homepage, static-asset (mime + `ETag` + 304), metadata-file
//! (xml / txt / rss / missing), parent-route index, content-repository
//! fallback, and not-found branches. Each request builds a
//! [`StaticContentState`] pointing the fixture [`AppContext`] at the temp dist
//! via a custom `PathsConfig`.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::IntoResponse;
use systemprompt_api::services::static_content::config::StaticContentMatcher;
use systemprompt_api::services::static_content::static_files::{
    StaticContentState, compute_etag, serve_static_content,
};
use systemprompt_files::FilesConfig;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::RouteClassifier;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with, fixture_db_pool,
};
use tempfile::TempDir;

async fn state_with_dist(
    matcher: StaticContentMatcher,
) -> anyhow::Result<(TempDir, StaticContentState)> {
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
        matcher: Arc::new(matcher),
        route_classifier: Arc::new(RouteClassifier::new(None)),
    };
    Ok((tmp, state))
}

fn dist_dir(state: &StaticContentState) -> std::path::PathBuf {
    state.ctx.app_paths().web().dist().to_path_buf()
}

async fn serve(
    state: &StaticContentState,
    uri: &str,
    headers: HeaderMap,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let resp = serve_static_content(
        State(state.clone()),
        uri.parse::<Uri>().expect("uri"),
        headers,
        None,
    )
    .await
    .into_response();
    let status = resp.status();
    let hdrs = resp.headers().clone();
    let body = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("body")
        .to_vec();
    (status, hdrs, body)
}

#[tokio::test]
async fn serves_homepage_index() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    std::fs::write(dist_dir(&state).join("index.html"), "<h1>home</h1>")?;

    let (status, hdrs, body) = serve(&state, "/", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hdrs.get(header::CACHE_CONTROL).unwrap(), "no-cache");
    assert!(hdrs.contains_key(header::ETAG));
    assert_eq!(String::from_utf8_lossy(&body), "<h1>home</h1>");
    Ok(())
}

#[tokio::test]
async fn homepage_returns_304_when_etag_matches() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let content = b"<h1>home</h1>";
    std::fs::write(dist_dir(&state).join("index.html"), content)?;
    let etag = compute_etag(content);

    let mut headers = HeaderMap::new();
    headers.insert(header::IF_NONE_MATCH, etag.parse().unwrap());
    let (status, _hdrs, body) = serve(&state, "/", headers).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
    Ok(())
}

#[tokio::test]
async fn homepage_missing_index_is_500() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let (status, _hdrs, _body) = serve(&state, "/", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}

#[tokio::test]
async fn serves_static_asset_with_resolved_mime() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let dist = dist_dir(&state);
    std::fs::create_dir_all(dist.join("assets"))?;
    std::fs::write(dist.join("assets/app.js"), "console.log(1)")?;
    std::fs::write(dist.join("assets/style.css"), "body{}")?;
    std::fs::write(dist.join("assets/logo.svg"), "<svg/>")?;

    for (path, mime) in [
        ("/assets/app.js", "application/javascript"),
        ("/assets/style.css", "text/css"),
        ("/assets/logo.svg", "image/svg+xml"),
    ] {
        let (status, hdrs, _body) = serve(&state, path, HeaderMap::new()).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(hdrs.get(header::CONTENT_TYPE).unwrap(), mime, "{path}");
        assert_eq!(
            hdrs.get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable",
            "{path}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn missing_static_asset_is_404() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let (status, _hdrs, _body) = serve(&state, "/assets/nope.js", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn serves_metadata_files_and_rss_mime() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let dist = dist_dir(&state);
    std::fs::write(dist.join("sitemap.xml"), "<urlset/>")?;
    std::fs::write(dist.join("robots.txt"), "User-agent: *")?;
    std::fs::write(dist.join("feed.xml"), "<rss/>")?;

    let (status, hdrs, _b) = serve(&state, "/sitemap.xml", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hdrs.get(header::CONTENT_TYPE).unwrap(), "application/xml");
    assert_eq!(
        hdrs.get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=3600"
    );

    let (status, hdrs, _b) = serve(&state, "/robots.txt", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hdrs.get(header::CONTENT_TYPE).unwrap(), "text/plain");

    let (status, hdrs, _b) = serve(&state, "/feed.xml", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        hdrs.get(header::CONTENT_TYPE).unwrap(),
        "application/rss+xml; charset=utf-8"
    );
    Ok(())
}

#[tokio::test]
async fn missing_metadata_file_is_404() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let (status, _hdrs, _b) = serve(&state, "/llms.txt", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn serves_parent_route_index() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let dist = dist_dir(&state);
    std::fs::create_dir_all(dist.join("about"))?;
    std::fs::write(dist.join("about/index.html"), "<h1>about</h1>")?;

    let (status, hdrs, body) = serve(&state, "/about", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hdrs.get(header::CONTENT_TYPE).unwrap(), "text/html");
    assert_eq!(String::from_utf8_lossy(&body), "<h1>about</h1>");
    Ok(())
}

#[tokio::test]
async fn unmatched_path_is_404() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    let (status, _hdrs, body) = serve(&state, "/no/such/page", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(String::from_utf8_lossy(&body).contains("404"));
    Ok(())
}

#[tokio::test]
async fn custom_404_html_is_served() -> anyhow::Result<()> {
    let (_tmp, state) = state_with_dist(StaticContentMatcher::empty()).await?;
    std::fs::write(dist_dir(&state).join("404.html"), "<h1>custom 404</h1>")?;

    let (status, hdrs, body) = serve(&state, "/nope-page", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(hdrs.get(header::CONTENT_TYPE).unwrap(), "text/html");
    assert!(String::from_utf8_lossy(&body).contains("custom 404"));
    Ok(())
}

#[tokio::test]
async fn content_page_matcher_falls_back_to_repo_lookup() -> anyhow::Result<()> {
    let tmp_cfg = TempDir::new()?;
    let cfg_path = tmp_cfg.path().join("content.yaml");
    std::fs::write(
        &cfg_path,
        concat!(
            "content_sources:\n",
            "  blog:\n",
            "    path: blog\n",
            "    source_id: blog\n",
            "    category_id: blog\n",
            "    enabled: true\n",
            "    sitemap:\n",
            "      enabled: true\n",
            "      url_pattern: \"/blog/{slug}\"\n",
            "      priority: 0.5\n",
            "      changefreq: daily\n",
        ),
    )?;
    let matcher = StaticContentMatcher::from_config(cfg_path.to_str().unwrap())?;
    assert!(matcher.matches("/blog/my-post").is_some());

    let (_tmp, state) = state_with_dist(matcher).await?;
    let (status, _hdrs, _body) = serve(&state, "/blog/unknown-post", HeaderMap::new()).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "unknown slug on an empty content DB falls through to 404"
    );
    Ok(())
}

#[test]
fn compute_etag_is_stable_and_content_sensitive() {
    let a = compute_etag(b"hello");
    let b = compute_etag(b"hello");
    let c = compute_etag(b"world");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.starts_with('"') && a.ends_with('"'));
}
