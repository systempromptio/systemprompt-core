//! Full-router coverage for the Slack inbound surface.
//!
//! Drives the real `slack_router` via `tower::ServiceExt::oneshot`. The
//! config-free edges (malformed body, unknown workspace, tampered signature,
//! `url_verification`) need no agent backend; the signed slash-command
//! happy-path runs the whole spawned dispatch and asserts the rendered Block
//! Kit reaches the captured `response_url` (a loopback wiremock).

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use systemprompt_slack::signature::sign;

use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID, agent_reply_response_json,
    ensure_messaging_bootstrap, install_test_signing_key, seed_agent_backend, test_app_context,
    test_db_pool,
};
use systemprompt_traits::DrainOutcome;
use tower::ServiceExt;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::body_to_string;

// A fixture context whose `config.yaml` carries the messaging Slack app, so
// `resolve_app` and the signing-secret lookup resolve.
async fn messaging_ctx() -> anyhow::Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_messaging_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &b.database_url);
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

fn router(ctx: &std::sync::Arc<systemprompt_runtime::AppContext>) -> axum::Router {
    systemprompt_api::routes::slack::slack_router().with_state((**ctx).clone())
}

#[tokio::test]
async fn malformed_event_body_is_bad_request() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let req = Request::builder()
        .method("POST")
        .uri("/events")
        .header("content-type", "application/json")
        .body(Body::from("not json"))
        .expect("request");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn unknown_workspace_is_acked() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = "command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id=T_UNKNOWN&response_url=https%3A%2F%2Fexample.invalid%2Fr";
    // Unknown workspace short-circuits to a 200 ack before signature checks.
    let req = signed_post("/commands", body, "irrelevant");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn tampered_signature_is_unauthorized() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = format!(
        "command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id={TEST_SLACK_WORKSPACE_ID}&response_url=https%3A%2F%2Fexample.invalid%2Fr"
    );
    let req = signed_post("/commands", &body, "the-wrong-secret");
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn url_verification_challenge_is_echoed() -> anyhow::Result<()> {
    let (_pool, ctx) = messaging_ctx().await?;
    let body = r#"{"type":"url_verification","challenge":"chal-123"}"#;
    let req = signed_post("/events", body, TEST_SLACK_SIGNING_SECRET);
    let resp = router(&ctx).oneshot(req).await?;
    let (status, text) = body_to_string(resp).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("chal-123"), "challenge echoed: {text}");
    Ok(())
}

#[tokio::test]
async fn signed_slash_command_dispatches_and_posts_to_response_url() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    install_test_signing_key();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &b.database_url);

    let agent = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(agent_reply_response_json("dispatched reply")),
        )
        .mount(&agent)
        .await;
    seed_agent_backend(&pool, &agent).await?;

    let response_hook = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "ok": true })))
        .expect(1)
        .mount(&response_hook)
        .await;

    let response_url = format!("{}/slack/respond", response_hook.uri());
    let body = format!(
        "command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id={TEST_SLACK_WORKSPACE_ID}&response_url={}",
        urlencode(&response_url)
    );
    let req = signed_post("/commands", &body, TEST_SLACK_SIGNING_SECRET);
    let resp = router(&ctx).oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK, "the route acks immediately");

    let posted = wait_for_request(&ctx, &response_hook).await;
    let body = String::from_utf8_lossy(&posted);
    assert!(
        body.contains("dispatched reply"),
        "rendered Block Kit carries the agent reply: {body}"
    );
    Ok(())
}

fn urlencode(s: &str) -> String {
    s.replace(':', "%3A").replace('/', "%2F")
}

// The reply comes from a background task running the full dispatch pipeline
// (identity linking, authz, proxy round-trip); under a loaded shard that has
// been observed to stall past 30s, so the drain bound must dwarf it.
async fn wait_for_request(ctx: &AppContext, server: &MockServer) -> Vec<u8> {
    assert_eq!(
        ctx.background_tasks().drain(Duration::from_secs(120)).await,
        DrainOutcome::Drained,
        "the reply task must finish within the drain bound"
    );
    server
        .received_requests()
        .await
        .and_then(|reqs| reqs.into_iter().next())
        .expect("the drained reply task posted to the response hook")
        .body
}

async fn coverage_command_reply(
    agent_response: serde_json::Value,
    denied: bool,
    hook_status: u16,
) -> anyhow::Result<serde_json::Value> {
    let b = ensure_messaging_bootstrap();
    install_test_signing_key();
    let pool = test_db_pool().await;
    let ctx = if denied {
        systemprompt_test_fixtures::fixture_app_context_with_hook(
            &pool,
            &b.database_url,
            Arc::new(systemprompt_security::authz::DenyAllHook::null()),
        )?
    } else {
        test_app_context(&pool, &b.database_url)
    };
    let agent = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(agent_response))
        .expect(if denied { 0 } else { 1 })
        .mount(&agent)
        .await;
    seed_agent_backend(&pool, &agent).await?;
    let hook = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(hook_status)
                .set_body_json(serde_json::json!({"ok": hook_status == 200})),
        )
        .expect(1)
        .mount(&hook)
        .await;
    let user = uuid::Uuid::new_v4().simple();
    let body = format!(
        "command=%2Fask&text=hi&user_id=U_{user}&channel_id=C1&team_id={TEST_SLACK_WORKSPACE_ID}&response_url={}",
        urlencode(&hook.uri())
    );
    let response = router(&ctx)
        .oneshot(signed_post("/commands", &body, TEST_SLACK_SIGNING_SECRET))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(serde_json::from_slice(
        &wait_for_request(&ctx, &hook).await,
    )?)
}

#[tokio::test]
async fn coverage_slack_denial_is_ephemeral_and_never_reaches_the_agent() -> anyhow::Result<()> {
    let body =
        coverage_command_reply(agent_reply_response_json("must not execute"), true, 200).await?;
    assert_eq!(body["response_type"], "ephemeral");
    assert!(body["blocks"].to_string().contains('⛔'));
    Ok(())
}

#[tokio::test]
async fn coverage_slack_agent_failure_posts_an_ephemeral_error() -> anyhow::Result<()> {
    let body = coverage_command_reply(
        systemprompt_test_fixtures::agent_error_response_json(-32603, "private backend details"),
        false,
        200,
    )
    .await?;
    assert_eq!(body["response_type"], "ephemeral");
    assert!(!body["blocks"].as_array().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn coverage_slack_empty_reply_has_a_visible_placeholder() -> anyhow::Result<()> {
    let body = coverage_command_reply(agent_reply_response_json("  \n"), false, 200).await?;
    assert_eq!(body["response_type"], "in_channel");
    assert!(body["blocks"].to_string().contains("(no response)"));
    Ok(())
}
