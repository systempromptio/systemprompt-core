//! Events-API and interactivity coverage for the Slack inbound surface.
//!
//! `routes_slack` drives the slash-command happy path and the config-free
//! edges; this suite exercises the `EventCallback` branch of `/events` (bot
//! echo, non-message kinds, missing channel/user, and the dispatching message)
//! and the `/interactivity` handler branches (missing/invalid payload, unknown
//! workspace). The dispatching interactivity path shares the slash-command
//! happy path's coverage of `spawn_reply` and is left to `routes_slack` to
//! avoid a second concurrent dispatch holding the shared pool.

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

fn signed_post(path: &str, body: &str, secret: &str, content_type: &str) -> Request<Body> {
    let ts = now_ts();
    let signature = sign(secret.as_bytes(), &ts, body.as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", content_type)
        .header("x-slack-request-timestamp", ts)
        .header("x-slack-signature", signature)
        .body(Body::from(body.to_owned()))
        .expect("request build")
}

fn router(ctx: &Arc<AppContext>) -> axum::Router {
    systemprompt_api::routes::slack::slack_router().with_state((**ctx).clone())
}

fn event_body(kind: &str, extra: serde_json::Value) -> String {
    let mut event = serde_json::json!({ "type": kind });
    if let (Some(obj), Some(ex)) = (event.as_object_mut(), extra.as_object()) {
        for (k, v) in ex {
            obj.insert(k.clone(), v.clone());
        }
    }
    serde_json::json!({
        "type": "event_callback",
        "team_id": TEST_SLACK_WORKSPACE_ID,
        "event": event,
    })
    .to_string()
}

#[tokio::test]
async fn event_callback_bot_echo_is_acked() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = event_body(
        "message",
        serde_json::json!({ "bot_id": "B1", "channel": "C1", "user": "U1", "text": "hi" }),
    );
    let req = signed_post(
        "/events",
        &body,
        TEST_SLACK_SIGNING_SECRET,
        "application/json",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn event_callback_non_message_kind_is_acked() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = event_body(
        "reaction_added",
        serde_json::json!({ "channel": "C1", "user": "U1" }),
    );
    let req = signed_post(
        "/events",
        &body,
        TEST_SLACK_SIGNING_SECRET,
        "application/json",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn event_callback_missing_channel_is_acked() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = event_body("message", serde_json::json!({ "user": "U1", "text": "hi" }));
    let req = signed_post(
        "/events",
        &body,
        TEST_SLACK_SIGNING_SECRET,
        "application/json",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn event_callback_unknown_workspace_is_acked() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = serde_json::json!({
        "type": "event_callback",
        "team_id": "T_UNKNOWN",
        "event": { "type": "message", "channel": "C1", "user": "U1", "text": "hi" },
    })
    .to_string();
    let req = signed_post("/events", &body, "irrelevant", "application/json");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn event_callback_wrong_signature_is_unauthorized() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = event_body(
        "message",
        serde_json::json!({ "channel": "C1", "user": "U1", "text": "hi" }),
    );
    let req = signed_post("/events", &body, "the-wrong-secret", "application/json");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn event_callback_message_dispatches_and_acks() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = event_body(
        "message",
        serde_json::json!({ "channel": "C1", "user": "U1", "text": "hi there" }),
    );
    let req = signed_post(
        "/events",
        &body,
        TEST_SLACK_SIGNING_SECRET,
        "application/json",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "route acks within Slack's window"
    );
    Ok(())
}

#[tokio::test]
async fn interactivity_missing_payload_is_bad_request() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let req = signed_post(
        "/interactivity",
        "nopayload=1",
        TEST_SLACK_SIGNING_SECRET,
        "application/x-www-form-urlencoded",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn interactivity_invalid_payload_json_is_bad_request() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = format!("payload={}", urlencode("not json"));
    let req = signed_post(
        "/interactivity",
        &body,
        TEST_SLACK_SIGNING_SECRET,
        "application/x-www-form-urlencoded",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn interactivity_unknown_workspace_is_acked() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let payload = serde_json::json!({
        "type": "block_actions",
        "user": { "id": "U1" },
        "channel": { "id": "C1" },
        "team": { "id": "T_UNKNOWN" },
        "actions": [ { "action_id": "a", "value": "hi" } ],
    })
    .to_string();
    let body = format!("payload={}", urlencode(&payload));
    let req = signed_post(
        "/interactivity",
        &body,
        "irrelevant",
        "application/x-www-form-urlencoded",
    );
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
