//! `/commands` guard branches and the `/interactivity` signature / team-routing
//! edges not driven by `routes_slack` / `routes_slack_more`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_slack::signature::sign;
use systemprompt_test_fixtures::{
    TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID, ensure_messaging_bootstrap,
    fixture_app_context, fixture_db_pool,
};
use tower::ServiceExt;

async fn messaging_ctx() -> anyhow::Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok((pool, ctx))
}

fn now_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        .to_string()
}

fn signed_post(path: &str, body: &str, secret: &str) -> Request<Body> {
    let ts = now_ts();
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

fn router(ctx: &Arc<AppContext>) -> axum::Router {
    systemprompt_api::routes::slack::slack_router().with_state((**ctx).clone())
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn command_body(team_id: &str) -> String {
    format!(
        "command=%2Fask&text=hello&user_id=U1&channel_id=C1&team_id={team_id}&response_url={}",
        urlencode("https://hooks.slack.invalid/respond")
    )
}

#[tokio::test]
async fn command_missing_required_fields_is_bad_request() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let req = signed_post(
        "/commands",
        "command=%2Fask&text=hello",
        TEST_SLACK_SIGNING_SECRET,
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn command_unknown_workspace_is_acked_without_dispatch() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let req = signed_post("/commands", &command_body("T_UNKNOWN"), "irrelevant");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn command_wrong_signature_is_unauthorized() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let req = signed_post(
        "/commands",
        &command_body(TEST_SLACK_WORKSPACE_ID),
        "the-wrong-secret",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn interactivity_wrong_signature_for_known_workspace_is_unauthorized() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let payload = serde_json::json!({
        "type": "block_actions",
        "user": { "id": "U1" },
        "channel": { "id": "C1" },
        "team": { "id": TEST_SLACK_WORKSPACE_ID },
        "actions": [ { "action_id": "a", "value": "hi" } ],
    })
    .to_string();
    let body = format!("payload={}", urlencode(&payload));
    let req = signed_post("/interactivity", &body, "the-wrong-secret");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn interactivity_without_channel_routes_by_team_and_acks() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let payload = serde_json::json!({
        "type": "block_actions",
        "user": { "id": "U1" },
        "team": { "id": TEST_SLACK_WORKSPACE_ID },
        "actions": [ { "action_id": "a", "value": "channel-less action" } ],
        "response_url": "https://hooks.slack.invalid/respond",
    })
    .to_string();
    let body = format!("payload={}", urlencode(&payload));
    let req = signed_post("/interactivity", &body, TEST_SLACK_SIGNING_SECRET);
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "team-level routing must still ack within Slack's window"
    );
    Ok(())
}
