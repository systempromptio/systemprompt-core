//! Tail branches of the Slack handlers: event callbacks a bot echo, a
//! non-message kind or a missing channel only ack; a bad signature is
//! refused; an interactivity form without a payload, or with a payload that
//! is not JSON, is a bad request; an unknown workspace only acks.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_slack::signature::sign;
use systemprompt_test_fixtures::{
    TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID, ensure_messaging_bootstrap,
    test_app_context,
};
use tower::ServiceExt;

fn slack_router(ctx: &systemprompt_runtime::AppContext) -> axum::Router {
    super::common::slack_test_router(ctx)
}

fn signed_slack_post(path: &str, body: &str, secret: &str) -> Request<Body> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        .to_string();
    let signature = sign(secret.as_bytes(), &ts, body.as_bytes()).expect("sign");
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
    Ok(test_app_context(&b.database_url).await)
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
    let payload =
        r#"{"type":"block_actions","user":{"id":"U_X"},"team":{"id":"T_UNKNOWN"},"actions":[]}"#;
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
