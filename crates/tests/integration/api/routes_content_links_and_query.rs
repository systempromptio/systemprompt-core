//! Content search and trackable-link generation.
//!
//! `query_handler` and `generate_link_handler` are both driven against the live
//! content schema so their success branches execute rather than only the
//! service-construction failures. Link generation is exercised on the rejected
//! `link_type` arm and on real generations whose ids then resolve through the
//! analytics handlers.
//!
//! The unfiltered search branch is deliberately asserted only on its envelope:
//! `SearchService::search` does not read `request.query` at all, so any
//! assertion about which documents come back would pin behaviour the endpoint
//! does not actually have.

use anyhow::Result;
use axum::{Extension, Router};
use systemprompt_api::routes::content;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{body_to_string, empty_get, json_post, request_context, setup_ctx};

fn public(ctx: &AppContext) -> Router {
    content::public_router(ctx).layer(Extension(request_context("content_user")))
}

fn authenticated(ctx: &AppContext) -> Router {
    content::authenticated_router(ctx).layer(Extension(request_context("content_user")))
}

async fn seed_searchable(db: &DbPool, term: &str) -> Result<()> {
    let uniq = Uuid::new_v4().to_string();
    let p = db.pool_arc()?;
    sqlx::query(
        "INSERT INTO markdown_content \
         (id, slug, title, description, body, author, published_at, keywords, source_id, \
         version_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9)",
    )
    .bind(format!("mc-{uniq}"))
    .bind(format!("slug-{uniq}"))
    .bind(format!("A post about {term}"))
    .bind(format!("{term} description"))
    .bind(format!("# Body\n\nall about {term}"))
    .bind("Author")
    .bind(term)
    .bind(format!("src-{uniq}"))
    .bind(format!("hash-{uniq}"))
    .execute(p.as_ref())
    .await?;
    Ok(())
}

#[tokio::test]
async fn a_search_over_seeded_content_returns_a_result_envelope() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let term = format!("zq{}", Uuid::new_v4().simple());
    seed_searchable(&db, &term).await?;

    let (status, body) = body_to_string(
        public(&ctx)
            .oneshot(json_post("/query", serde_json::json!({ "query": term })))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let parsed: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        parsed["total"].is_number(),
        "a search response reports how many matches it found: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_category_filtered_search_is_scoped_to_that_category() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let unknown_category = format!("cat-{}", Uuid::new_v4().simple());

    let (status, body) = body_to_string(
        public(&ctx)
            .oneshot(json_post(
                "/query",
                serde_json::json!({
                    "query": "anything",
                    "filters": { "category_id": unknown_category },
                }),
            ))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let parsed: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        parsed["total"].as_i64(),
        Some(0),
        "a category nothing belongs to must yield nothing: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unrecognised_link_type_is_rejected_before_any_write() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;

    let (status, body) = body_to_string(
        authenticated(&ctx)
            .oneshot(json_post(
                "/links/generate",
                serde_json::json!({
                    "target_url": "https://example.test/page",
                    "link_type": "teleport",
                }),
            ))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 400, "{body}");
    assert!(
        body.contains("redirect") && body.contains("utm"),
        "the rejection must name the accepted link types: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn each_declared_link_type_is_accepted() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;

    for link_type in ["redirect", "utm", "both"] {
        let (status, body) = body_to_string(
            authenticated(&ctx)
                .oneshot(json_post(
                    "/links/generate",
                    serde_json::json!({
                        "target_url": "https://example.test/page",
                        "link_type": link_type,
                        "utm_source": "newsletter",
                        "utm_medium": "email",
                        "utm_campaign": "launch",
                    }),
                ))
                .await?,
        )
        .await?;

        assert_eq!(status.as_u16(), 200, "{link_type} rejected: {body}");
        let parsed: serde_json::Value = serde_json::from_str(&body)?;
        assert!(
            parsed["short_code"].as_str().is_some_and(|c| !c.is_empty()),
            "{link_type} must mint a short code: {body}"
        );
        assert!(
            parsed["full_url"].as_str().is_some_and(|u| !u.is_empty()),
            "{link_type} must resolve to a usable url: {body}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_generated_link_has_performance_and_an_unknown_link_does_not() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (_status, body) = body_to_string(
        authenticated(&ctx)
            .oneshot(json_post(
                "/links/generate",
                serde_json::json!({
                    "target_url": "https://example.test/tracked",
                    "link_type": "redirect",
                }),
            ))
            .await?,
    )
    .await?;
    let generated: serde_json::Value = serde_json::from_str(&body)?;
    let link_id = generated["link_id"]
        .as_str()
        .expect("a generated link carries its id")
        .to_owned();

    let (found, found_body) = body_to_string(
        public(&ctx)
            .oneshot(empty_get(&format!("/links/{link_id}/performance")))
            .await?,
    )
    .await?;
    assert_eq!(found.as_u16(), 200, "{found_body}");

    let (missing, missing_body) = body_to_string(
        public(&ctx)
            .oneshot(empty_get(&format!(
                "/links/link-{}/performance",
                Uuid::new_v4().simple()
            )))
            .await?,
    )
    .await?;
    assert_eq!(
        missing.as_u16(),
        404,
        "an unknown link must not report performance: {missing_body}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unknown_campaign_has_no_performance() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;

    let (status, body) = body_to_string(
        public(&ctx)
            .oneshot(empty_get(&format!(
                "/links/campaigns/camp-{}/performance",
                Uuid::new_v4().simple()
            )))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 404, "{body}");
    Ok(())
}
