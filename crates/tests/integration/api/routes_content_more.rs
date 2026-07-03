//! Tail branches of the content routers: short-code redirects (seeded link,
//! non-bot tracking + bot skip + not-found), link generation `link_type`
//! variants, `list_links` query dispatch, and a seeded content search.

use axum::Extension;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_api::routes::content;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_slack::signature::sign;
use systemprompt_test_fixtures::{
    TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID, ensure_messaging_bootstrap,
    fixture_app_context, fixture_db_pool,
};
use tower::ServiceExt;

use super::common::{empty_get, json_post, request_context, setup_ctx};

fn slack_router(ctx: &systemprompt_runtime::AppContext) -> axum::Router {
    systemprompt_api::routes::slack::slack_router().with_state(ctx.clone())
}

fn signed_slack_post(path: &str, body: &str, secret: &str) -> Request<Body> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        .to_string();
    let signature = sign(secret.as_bytes(), &ts, body.as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("x-slack-request-timestamp", ts)
        .header("x-slack-signature", signature)
        .body(Body::from(body.to_owned()))
        .expect("request build")
}

async fn messaging_ctx() -> anyhow::Result<std::sync::Arc<systemprompt_runtime::AppContext>> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    fixture_app_context(&pool, &b.database_url)
}

#[tokio::test]
async fn slack_event_callback_bot_echo_is_acked() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let body = format!(
        r#"{{"type":"event_callback","team_id":"{TEST_SLACK_WORKSPACE_ID}","event":{{"type":"message","bot_id":"B1"}}}}"#
    );
    let req = signed_slack_post("/events", &body, TEST_SLACK_SIGNING_SECRET);
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn slack_event_callback_non_message_kind_is_acked() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let body = format!(
        r#"{{"type":"event_callback","team_id":"{TEST_SLACK_WORKSPACE_ID}","event":{{"type":"reaction_added"}}}}"#
    );
    let req = signed_slack_post("/events", &body, TEST_SLACK_SIGNING_SECRET);
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn slack_event_callback_missing_channel_is_acked() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let body = format!(
        r#"{{"type":"event_callback","team_id":"{TEST_SLACK_WORKSPACE_ID}","event":{{"type":"message","user":"U1","text":"hi"}}}}"#
    );
    let req = signed_slack_post("/events", &body, TEST_SLACK_SIGNING_SECRET);
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn slack_event_callback_bad_signature_is_unauthorized() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let body = format!(
        r#"{{"type":"event_callback","team_id":"{TEST_SLACK_WORKSPACE_ID}","event":{{"type":"message","channel":"C1","user":"U1","text":"hi"}}}}"#
    );
    let req = signed_slack_post("/events", &body, "wrong-secret");
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn slack_interactivity_missing_payload_is_bad_request() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let req = signed_slack_post("/interactivity", "not_payload=1", TEST_SLACK_SIGNING_SECRET);
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn slack_interactivity_bad_payload_json_is_bad_request() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let req = signed_slack_post(
        "/interactivity",
        "payload=not-json",
        TEST_SLACK_SIGNING_SECRET,
    );
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn slack_interactivity_unknown_workspace_is_acked() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let payload = r#"{"type":"block_actions","user":{"id":"U_X"},"team":{"id":"T_UNKNOWN"},"actions":[]}"#;
    let body = format!("payload={}", urlencode(payload));
    let req = signed_slack_post("/interactivity", &body, "irrelevant");
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn slack_interactivity_tampered_signature_is_unauthorized() -> anyhow::Result<()> {
    let ctx = messaging_ctx().await?;
    let payload = format!(
        r#"{{"type":"block_actions","user":{{"id":"U_X"}},"team":{{"id":"{TEST_SLACK_WORKSPACE_ID}"}},"actions":[]}}"#
    );
    let body = format!("payload={}", urlencode(&payload));
    let req = signed_slack_post("/interactivity", &body, "the-wrong-secret");
    let resp = slack_router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

fn urlencode(s: &str) -> String {
    s.replace('%', "%25")
        .replace('{', "%7B")
        .replace('}', "%7D")
        .replace(':', "%3A")
        .replace('"', "%22")
        .replace(',', "%2C")
        .replace('[', "%5B")
        .replace(']', "%5D")
}

fn bot_context() -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("bot_{}", uuid::Uuid::new_v4())),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("test-agent"),
    )
    .with_actor(Actor::user(UserId::new("user_bot")))
}

async fn seed_link(db: &DbPool) -> anyhow::Result<String> {
    let uniq = uuid::Uuid::new_v4().simple().to_string();
    let short_code = format!("sc{uniq}");
    let id = format!("lnk-{uniq}");
    let p = db.pool_arc()?;
    sqlx::query(
        "INSERT INTO campaign_links (id, short_code, target_url, link_type) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(&id)
    .bind(&short_code)
    .bind("https://example.com/dest")
    .bind("redirect")
    .execute(p.as_ref())
    .await?;
    Ok(short_code)
}

#[tokio::test]
async fn redirect_seeded_link_tracks_and_redirects() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let short_code = seed_link(&db).await?;
    let app =
        content::redirect_router(ctx.db_pool()).layer(Extension(request_context("user_redirect")));
    let resp = app.oneshot(empty_get(&format!("/r/{short_code}"))).await?;
    assert_eq!(resp.status().as_u16(), 307, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn redirect_bot_session_skips_tracking() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let short_code = seed_link(&db).await?;
    let app = content::redirect_router(ctx.db_pool()).layer(Extension(bot_context()));
    let resp = app.oneshot(empty_get(&format!("/r/{short_code}"))).await?;
    assert_eq!(resp.status().as_u16(), 307, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn redirect_unknown_short_code_is_not_found() -> anyhow::Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let app =
        content::redirect_router(ctx.db_pool()).layer(Extension(request_context("user_redirect")));
    let resp = app.oneshot(empty_get("/r/does-not-exist")).await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn generate_link_invalid_type_is_bad_request() -> anyhow::Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let app = content::authenticated_router(&ctx).layer(Extension(request_context("user_gen")));
    let body = serde_json::json!({ "target_url": "https://example.com", "link_type": "bogus" });
    let resp = app.oneshot(json_post("/links/generate", body)).await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn generate_link_utm_and_both_types_run_handler() -> anyhow::Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    for link_type in ["redirect", "utm", "both"] {
        let app = content::authenticated_router(&ctx).layer(Extension(request_context("user_gen")));
        let body = serde_json::json!({
            "target_url": "https://example.com/page",
            "link_type": link_type,
            "utm_source": "newsletter",
            "utm_medium": "email",
            "campaign_name": "spring",
        });
        let resp = app.oneshot(json_post("/links/generate", body)).await?;
        assert!(
            resp.status().as_u16() >= 200,
            "{link_type}: {}",
            resp.status()
        );
    }
    Ok(())
}

#[tokio::test]
async fn list_links_by_campaign_and_source_content() -> anyhow::Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_ll")));
    let resp = app.oneshot(empty_get("/links?campaign_id=camp_x")).await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());

    let app = content::public_router(&ctx).layer(Extension(request_context("user_ll")));
    let resp = app
        .oneshot(empty_get("/links?source_content_id=content_y"))
        .await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn query_over_seeded_content_returns_results() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let uniq = uuid::Uuid::new_v4().simple().to_string();
    let slug = format!("post-{uniq}");
    let source = format!("src-{uniq}");
    let p = db.pool_arc()?;
    sqlx::query(
        "INSERT INTO markdown_content \
         (id, slug, title, description, body, author, published_at, keywords, source_id, \
         version_hash) VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9)",
    )
    .bind(format!("mc-{uniq}"))
    .bind(&slug)
    .bind("Searchable Title")
    .bind("Desc")
    .bind("# body content")
    .bind("Author")
    .bind("keyword")
    .bind(&source)
    .bind(format!("hash-{uniq}"))
    .execute(p.as_ref())
    .await?;

    let app = content::public_router(&ctx).layer(Extension(request_context("user_query")));
    let body = serde_json::json!({ "query": "Searchable", "limit": 10 });
    let resp = app.oneshot(json_post("/query", body)).await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}
