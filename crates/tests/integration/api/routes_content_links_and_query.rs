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
use systemprompt_test_fixtures::{DisposableDb, fixture_app_context};
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

#[tokio::test]
async fn search_database_failure_is_a_json_500_and_recovers_after_schema_repair() -> Result<()> {
    let owned = DisposableDb::installed("content_query_failure").await?;
    let db = owned.pool().await?;
    let ctx = fixture_app_context(&db, owned.url())?;
    let raw = db.pool_arc()?;
    sqlx::query("ALTER TABLE markdown_content RENAME TO markdown_content_unavailable")
        .execute(raw.as_ref())
        .await?;

    let (status, body) = body_to_string(
        public(&ctx)
            .oneshot(json_post("/query", serde_json::json!({"query": "failure"})))
            .await?,
    )
    .await?;
    assert_eq!(status.as_u16(), 500, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        error["error"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "{body}"
    );

    sqlx::query("ALTER TABLE markdown_content_unavailable RENAME TO markdown_content")
        .execute(raw.as_ref())
        .await?;
    let (status, body) = body_to_string(
        public(&ctx)
            .oneshot(json_post(
                "/query",
                serde_json::json!({"query": "repaired"}),
            ))
            .await?,
    )
    .await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    drop(ctx);
    raw.close().await;
    drop(raw);
    drop(db);
    owned.drop_now().await;
    Ok(())
}

async fn response_json(
    app: Router,
    request: axum::http::Request<axum::body::Body>,
) -> Result<(http::StatusCode, serde_json::Value)> {
    let (status, body) = body_to_string(app.oneshot(request).await?).await?;
    let value = serde_json::from_str(&body)?;
    Ok((status, value))
}

async fn assert_link_reads_live(
    ctx: &AppContext,
    link: &str,
    campaign: &str,
    source: &str,
) -> Result<()> {
    let (status, performance) = response_json(
        public(ctx),
        empty_get(&format!("/links/{link}/performance")),
    )
    .await?;
    assert_eq!(status.as_u16(), 200, "{performance}");
    assert_eq!(performance["link_id"], link);
    assert_eq!(performance["click_count"], 1);

    let (status, performance) = response_json(
        public(ctx),
        empty_get(&format!("/links/campaigns/{campaign}/performance")),
    )
    .await?;
    assert_eq!(status.as_u16(), 200, "{performance}");
    assert_eq!(performance["campaign_id"], campaign);
    assert_eq!(performance["total_clicks"], 1);
    assert_eq!(performance["link_count"], 1);

    let (status, clicks) = response_json(
        public(ctx),
        empty_get(&format!("/links/{link}/clicks?limit=10&offset=0")),
    )
    .await?;
    assert_eq!(status.as_u16(), 200, "{clicks}");
    let clicks = clicks.as_array().expect("click response array");
    assert_eq!(clicks.len(), 1);
    assert_eq!(clicks[0]["link_id"], link);
    assert!(clicks[0]["id"].as_str().is_some_and(|id| !id.is_empty()));

    for uri in [
        format!("/links?campaign_id={campaign}"),
        format!("/links?source_content_id={source}"),
    ] {
        let (status, links) = response_json(public(ctx), empty_get(&uri)).await?;
        assert_eq!(status.as_u16(), 200, "{uri}: {links}");
        let links = links.as_array().expect("links response array");
        assert_eq!(links.len(), 1, "{uri}: {links:?}");
        assert_eq!(links[0]["id"], link);
        assert_eq!(links[0]["campaign_id"], campaign);
        assert_eq!(links[0]["source_content_id"], source);
    }

    let (status, journey) =
        response_json(public(ctx), empty_get("/links/journey?limit=10&offset=0")).await?;
    assert_eq!(status.as_u16(), 200, "{journey}");
    let journey = journey.as_array().expect("journey response array");
    assert_eq!(journey.len(), 1);
    assert_eq!(journey[0]["source_content_id"], source);
    assert_eq!(journey[0]["click_count"], 1);
    Ok(())
}

async fn assert_link_reads_outage(
    ctx: &AppContext,
    link: &str,
    campaign: &str,
    source: &str,
    database_url: &str,
) -> Result<()> {
    for uri in [
        format!("/links/{link}/performance"),
        format!("/links/campaigns/{campaign}/performance"),
        format!("/links/{link}/clicks?limit=10&offset=0"),
        format!("/links?campaign_id={campaign}"),
        format!("/links?source_content_id={source}"),
        "/links/journey?limit=10&offset=0".to_owned(),
    ] {
        let (status, body) = response_json(public(ctx), empty_get(&uri)).await?;
        assert_eq!(status.as_u16(), 500, "{uri}: {body}");
        assert!(
            body["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty()),
            "{uri}: {body}"
        );
        let rendered = body.to_string();
        assert!(!rendered.contains(database_url));
        assert!(!rendered.contains("postgres://"));
        assert!(!rendered.contains("password"));
    }
    Ok(())
}

#[tokio::test]
async fn link_analytics_reads_report_database_outage_and_recover_without_false_empty_results()
-> Result<()> {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let owned = DisposableDb::installed("link_read_outage").await?;
    let db = owned.pool().await?;
    let ctx = fixture_app_context(&db, owned.url())?;
    let click_user = systemprompt_identifiers::UserId::new("content_user");
    let click_session = systemprompt_identifiers::SessionId::generate();
    systemprompt_test_fixtures::seed_user_row(&db, &click_user, "content-user@outage.invalid")
        .await?;
    systemprompt_test_fixtures::seed_user_session(&db, &click_user, &click_session).await?;
    let click_context = systemprompt_models::RequestContext::new(
        click_session,
        systemprompt_identifiers::TraceId::generate(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::AgentName::try_new("link-outage").unwrap(),
    )
    .with_actor(systemprompt_identifiers::Actor::user(click_user));
    let campaign = format!("campaign-{}", Uuid::new_v4().simple());
    let source = systemprompt_content::repository::ContentRepository::new(&db)?
        .create(&systemprompt_content::models::CreateContentParams {
            slug: format!("outage-source-{}", Uuid::new_v4().simple()),
            locale: systemprompt_identifiers::LocaleCode::english(),
            title: "Outage source".to_owned(),
            description: "Source for link outage recovery".to_owned(),
            body: "Retained source body".to_owned(),
            author: "Test".to_owned(),
            published_at: chrono::Utc::now(),
            keywords: String::new(),
            kind: "article".to_owned(),
            image: None,
            category_id: None,
            source_id: systemprompt_identifiers::SourceId::new(format!(
                "outage-source-{}",
                Uuid::new_v4().simple()
            )),
            version_hash: format!("outage-hash-{}", Uuid::new_v4().simple()),
            links: serde_json::json!([]),
            public: true,
        })
        .await?
        .id
        .to_string();
    let (status, generated) = response_json(
        authenticated(&ctx),
        json_post(
            "/links/generate",
            serde_json::json!({
                "target_url": "https://example.test/outage-target",
                "link_type": "both",
                "campaign_id": campaign,
                "campaign_name": "outage campaign",
                "source_content_id": source,
                "source_page": "/outage-source",
                "utm_source": "test"
            }),
        ),
    )
    .await?;
    assert_eq!(status.as_u16(), 200, "{generated}");
    let link = generated["link_id"].as_str().expect("link id").to_owned();
    let short = generated["short_code"]
        .as_str()
        .expect("short code")
        .to_owned();
    let redirect = content::redirect_router(ctx.content_repositories())
        .layer(Extension(click_context))
        .oneshot(empty_get(&format!("/r/{short}")))
        .await?;
    assert!(redirect.status().is_redirection());
    assert_link_reads_live(&ctx, &link, &campaign, &source).await?;

    let raw = db.pool_arc()?;
    raw.close().await;
    assert_link_reads_outage(&ctx, &link, &campaign, &source, owned.url()).await?;
    drop(ctx);
    drop(raw);
    drop(db);

    let recovered = owned.pool().await?;
    let recovered_ctx = fixture_app_context(&recovered, owned.url())?;
    assert_link_reads_live(&recovered_ctx, &link, &campaign, &source).await?;
    let row: i64 = sqlx::query_scalar("SELECT count(*) FROM campaign_links WHERE id=$1")
        .bind(&link)
        .fetch_one(recovered.pool_arc()?.as_ref())
        .await?;
    assert_eq!(row, 1);
    drop(recovered_ctx);
    drop(recovered);
    owned.drop_now().await;
    Ok(())
}
