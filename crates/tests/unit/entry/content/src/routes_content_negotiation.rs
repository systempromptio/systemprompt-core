//! Content-negotiation branches of the public content routes: the Markdown
//! route answers only when negotiation is on and the trailing segment equals
//! the configured suffix, and JSON responses carry the `Link` alternate when
//! negotiation is on.

use anyhow::Result;
use axum::{Extension, Router};
use http_body_util::BodyExt;
use systemprompt_content_entry::http as content;
use systemprompt_database::DbPool;
use systemprompt_extension::router::{AcceptedFormat, AcceptedMediaType};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
};
use tower::ServiceExt;

use crate::common::{content_app, empty_get, request_context};

async fn negotiating_ctx() -> Result<(DbPool, std::sync::Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&b.database_url);
    config.content_negotiation.enabled = true;
    config.content_negotiation.markdown_suffix = ".md".to_owned();
    let ctx = fixture_app_context_with_config(config).await?;
    Ok((pool, ctx))
}

async fn seed_content(db: &DbPool) -> Result<(String, String)> {
    let uniq = uuid::Uuid::new_v4().to_string();
    let source = format!("neg-src-{uniq}");
    let slug = format!("neg-post-{uniq}");
    let p = db.pool();
    sqlx::query(
        "INSERT INTO markdown_content \
         (id, slug, title, description, body, author, published_at, keywords, source_id, \
         version_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9)",
    )
    .bind(format!("mc-{uniq}"))
    .bind(&slug)
    .bind("Negotiated Title")
    .bind("desc")
    .bind("# Negotiated body")
    .bind("Author")
    .bind("rust, negotiation")
    .bind(&source)
    .bind(format!("hash-{uniq}"))
    .execute(p.as_ref())
    .await?;
    Ok((source, slug))
}

fn public(ctx: &AppContext) -> Router {
    content_app(content::public_routes(), ctx)
        .layer(Extension(AcceptedFormat(AcceptedMediaType::Json)))
        .layer(Extension(request_context("neg_reader")))
}

#[tokio::test]
async fn json_response_carries_markdown_alternate_link_header() -> Result<()> {
    let (db, ctx) = negotiating_ctx().await?;
    let (source, slug) = seed_content(&db).await?;
    let resp = public(&ctx)
        .oneshot(empty_get(&format!("/{source}/{slug}")))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    let link = resp
        .headers()
        .get("link")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .expect("Link header present when negotiation enabled");
    assert!(
        link.contains(&format!("/api/v1/content/{source}/{slug}/md")),
        "{link}"
    );
    assert!(link.contains("rel=\"alternate\""), "{link}");
    assert!(link.contains("text/markdown"), "{link}");
    Ok(())
}

#[tokio::test]
async fn suffix_route_serves_markdown_when_negotiation_enabled() -> Result<()> {
    let (db, ctx) = negotiating_ctx().await?;
    let (source, slug) = seed_content(&db).await?;
    let resp = public(&ctx)
        .oneshot(empty_get(&format!("/{source}/{slug}/md")))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(ct.contains("markdown"), "{ct}");
    let body = resp.into_body().collect().await?.to_bytes();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("Negotiated body"), "{text}");
    assert!(text.contains("Negotiated Title"), "{text}");
    Ok(())
}

#[tokio::test]
async fn suffix_route_answers_not_found_when_negotiation_disabled() -> Result<()> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = fixture_app_context_with_config(fixture_config(&b.database_url)).await?;
    let (source, slug) = seed_content(&pool).await?;
    let resp = public(&ctx)
        .oneshot(empty_get(&format!("/{source}/{slug}/md")))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn suffix_route_answers_not_found_for_another_suffix() -> Result<()> {
    let (db, ctx) = negotiating_ctx().await?;
    let (source, slug) = seed_content(&db).await?;
    let resp = public(&ctx)
        .oneshot(empty_get(&format!("/{source}/{slug}/txt")))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}
