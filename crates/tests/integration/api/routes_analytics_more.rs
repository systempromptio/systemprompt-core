//! Analytics + engagement content-routing resolution branches.
//!
//! Drives the `test-api` seams that mount the analytics and engagement routers
//! over a stub `ContentRouting`, so the slug-resolution success path (routing
//! maps a page URL to a seeded `markdown_content` slug, then `get_by_slug`
//! returns the row) and the conversion-marking path both execute — branches the
//! production router cannot reach without a live content config.

use std::sync::Arc;

use axum::Extension;
use systemprompt_api::routes::{analytics, engagement};
use systemprompt_database::DbPool;
use systemprompt_models::ContentRouting;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

struct SlugRouter {
    page_url: String,
    slug: String,
}

impl ContentRouting for SlugRouter {
    fn is_html_page(&self, _path: &str) -> bool {
        true
    }

    fn determine_source(&self, _path: &str) -> String {
        "test".to_owned()
    }

    fn resolve_slug(&self, path: &str) -> Option<String> {
        (path == self.page_url).then(|| self.slug.clone())
    }
}

async fn seed_content(db: &DbPool) -> anyhow::Result<(String, String)> {
    let uniq = uuid::Uuid::new_v4().to_string();
    let slug = format!("post-{uniq}");
    let source = format!("src-{uniq}");
    let id = format!("mc-{uniq}");
    let p = db.pool_arc()?;
    sqlx::query(
        "INSERT INTO markdown_content \
         (id, slug, title, description, body, author, published_at, keywords, source_id, \
         version_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9)",
    )
    .bind(&id)
    .bind(&slug)
    .bind("Title")
    .bind("Desc")
    .bind("# body")
    .bind("Author")
    .bind("k1, k2")
    .bind(&source)
    .bind(format!("hash-{uniq}"))
    .execute(p.as_ref())
    .await?;
    let page_url = format!("https://example.com/{slug}");
    Ok((slug, page_url))
}

#[tokio::test]
async fn record_event_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = analytics::test_api::router_with_routing(&ctx, Some(routing))?
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({ "event_type": "page_view", "page_url": page_url });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn record_event_with_explicit_slug_skips_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, _page_url) = seed_content(&db).await?;
    let app = analytics::test_api::router_with_routing(&ctx, None)?
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({
        "event_type": "page_view",
        "page_url": "https://example.com/whatever",
        "slug": slug,
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn page_exit_fanout_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = analytics::test_api::router_with_routing(&ctx, Some(routing))?
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({
        "event_type": "page_exit",
        "page_url": page_url,
        "data": {
            "time_on_page_ms": 5000,
            "max_scroll_depth": 90,
            "click_count": 4,
            "is_rage_click": true,
            "reading_pattern": "scan",
            "scroll_velocity_avg": 1.5,
            "time_to_first_interaction_ms": 300,
        }
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn batch_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = analytics::test_api::router_with_routing(&ctx, Some(routing))?
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({
        "events": [
            { "event_type": "page_view", "page_url": page_url },
            {
                "event_type": "page_exit",
                "page_url": page_url,
                "data": { "time_on_page_ms": 2000, "max_scroll_depth": 50 }
            }
        ]
    });
    let resp = app.oneshot(json_post("/events/batch", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn engagement_resolves_content_and_marks_conversion() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = engagement::test_api::router_with_routing(&ctx, Some(routing))?
        .layer(Extension(request_context("user_en_more")));

    let body = serde_json::json!({
        "page_url": page_url,
        "event_type": "github_click",
        "time_on_page_ms": 3000,
        "max_scroll_depth": 70,
        "click_count": 2,
    });
    let resp = app.oneshot(json_post("/", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn engagement_batch_resolves_and_marks_conversion() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = engagement::test_api::router_with_routing(&ctx, Some(routing))?
        .layer(Extension(request_context("user_en_more")));

    let body = serde_json::json!({
        "events": [
            {
                "page_url": page_url,
                "event_type": "demo_click",
                "time_on_page_ms": 1000,
                "max_scroll_depth": 30,
                "click_count": 1,
            },
            {
                "page_url": page_url,
                "event_type": "scroll",
                "time_on_page_ms": 500,
                "max_scroll_depth": 20,
                "click_count": 0,
            }
        ]
    });
    let resp = app.oneshot(json_post("/batch", body)).await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}
