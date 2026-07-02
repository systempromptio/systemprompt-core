//! Content delivery handlers in `routes::content::blog`.
//!
//! Seeds a `markdown_content` row so the found-content branches (JSON, the
//! Markdown `Accept` path, and the dedicated `.md` handler) execute alongside
//! the not-found branches, which the bare router only reaches as 404s.

use anyhow::Result;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::{Extension, Router};
use systemprompt_api::routes::content;
use systemprompt_api::services::middleware::{AcceptedFormat, AcceptedMediaType};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;

use super::common::{empty_get, request_context, setup_ctx};

struct Seeded {
    source: String,
    slug: String,
}

async fn seed_content(db: &DbPool) -> Result<Seeded> {
    let uniq = uuid::Uuid::new_v4().to_string();
    let source = format!("src-{uniq}");
    let slug = format!("post-{uniq}");
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
    .bind("Test Title")
    .bind("A description")
    .bind("# Body\n\nsome markdown")
    .bind("Author Name")
    .bind("rust, testing, content")
    .bind(&source)
    .bind(format!("hash-{uniq}"))
    .execute(p.as_ref())
    .await?;
    Ok(Seeded { source, slug })
}

fn public(ctx: &AppContext) -> Router {
    content::public_router(ctx)
        .layer(Extension(AcceptedFormat(AcceptedMediaType::Json)))
        .layer(Extension(request_context("content_reader")))
}

#[tokio::test]
async fn get_content_returns_seeded_document_as_json() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let seeded = seed_content(&db).await?;
    let app = public(&ctx);
    let resp = app
        .oneshot(empty_get(&format!("/{}/{}", seeded.source, seeded.slug)))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_content_unknown_slug_is_not_found() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let seeded = seed_content(&db).await?;
    let app = public(&ctx);
    let resp = app
        .oneshot(empty_get(&format!("/{}/no-such-slug", seeded.source)))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_content_honours_markdown_accept_extension() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let seeded = seed_content(&db).await?;
    let app = content::public_router(&ctx)
        .layer(Extension(AcceptedFormat(AcceptedMediaType::Markdown)))
        .layer(Extension(request_context("content_reader")));
    let resp = app
        .oneshot(empty_get(&format!("/{}/{}", seeded.source, seeded.slug)))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(ct.contains("markdown"), "expected markdown, got {ct}");
    Ok(())
}

#[tokio::test]
async fn list_content_by_source_returns_seeded_row() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let seeded = seed_content(&db).await?;
    let app = public(&ctx);
    let resp = app
        .oneshot(empty_get(&format!("/{}", seeded.source)))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn markdown_handler_renders_found_document() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let seeded = seed_content(&db).await?;
    let resp = content::get_content_markdown_handler(
        State((*ctx).clone()),
        Extension(request_context("content_reader")),
        Path((seeded.source, format!("{}.md", seeded.slug))),
    )
    .await
    .into_response();
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn markdown_handler_missing_document_is_not_found() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let resp = content::get_content_markdown_handler(
        State((*ctx).clone()),
        Extension(request_context("content_reader")),
        Path(("ghost-src".to_owned(), "ghost.md".to_owned())),
    )
    .await
    .into_response();
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}
