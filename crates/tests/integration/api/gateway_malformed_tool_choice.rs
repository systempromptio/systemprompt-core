//! A malformed `tool_choice` must be refused before the upstream is called.
//!
//! The gateway's promise is to behave like the upstream API for the
//! client-visible payload. A string `tool_choice` on the Anthropic surface is
//! a client bug; answering it with a 200 spends the caller's quota, writes an
//! audit row for a request the real API would have refused, and hides the bug.
//! The mock upstream records every hit, so "nothing was dispatched" is an
//! assertion rather than an inference.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use serde_json::json;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::headers::SESSION_ID;
use systemprompt_test_fixtures::{
    AuthedFixture, TestBootstrap, fixture_app_context, fixture_db_pool, init_services_bootstrap,
    install_test_signing_key, seed_admin_credential,
};
use tokio::sync::OnceCell;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODEL: &str = "claude-tool-choice-fixture";
const SECRET_NAME: &str = "anthropic_api_key";

struct Harness {
    boot: TestBootstrap,
    upstream: MockServer,
}

static HARNESS: OnceCell<Harness> = OnceCell::const_new();

fn gateway_yaml(endpoint: &str) -> String {
    format!(
        r#"
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: {endpoint}
    api_key_secret: {SECRET_NAME}
    models:
      - id: {MODEL}
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
          cache_read_per_million: 0.0
gateway:
  enabled: true
  allow_unlisted_models: false
  routes:
    - id: tool-choice-fixture
      model_pattern: "{MODEL}"
      provider: anthropic
"#
    )
}

async fn harness() -> &'static Harness {
    HARNESS
        .get_or_init(|| async {
            // SAFETY: set before any bootstrap call; process-local under nextest.
            unsafe {
                std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", SECRET_NAME);
                std::env::set_var(SECRET_NAME, "test-provider-key");
            }

            let upstream = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/messages"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": "msg_should_never_be_reached",
                    "type": "message",
                    "role": "assistant",
                    "model": MODEL,
                    "content": [{"type": "text", "text": "dispatched"}],
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 1, "output_tokens": 1}
                })))
                .mount(&upstream)
                .await;

            let boot = init_services_bootstrap(&gateway_yaml(&upstream.uri()));
            Harness { boot, upstream }
        })
        .await
}

async fn app() -> Result<(Router, DbPool)> {
    let h = harness().await;
    install_test_signing_key();
    let pool = fixture_db_pool(&h.boot.database_url).await?;
    let ctx = fixture_app_context(&pool, &h.boot.database_url)?;
    Ok((
        gateway_router(&ctx).expect("gateway router available"),
        pool,
    ))
}

async fn credential(pool: &DbPool) -> Result<AuthedFixture> {
    seed_admin_credential(pool, &format!("tc-{}@example.invalid", Uuid::new_v4())).await
}

fn messages_post(cred: &AuthedFixture, tool_choice: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri("/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", cred.jwt.as_str()),
        )
        .header(SESSION_ID, cred.session_id.as_str())
        .body(Body::from(
            json!({
                "model": MODEL,
                "max_tokens": 64,
                "messages": [{"role": "user", "content": "hi"}],
                "tools": [{"name": "list_files", "input_schema": {}}],
                "tool_choice": tool_choice,
            })
            .to_string(),
        ))
        .expect("request must build")
}

#[tokio::test]
async fn a_string_tool_choice_is_refused_and_never_dispatched() -> Result<()> {
    let h = harness().await;
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;

    let (status, body) =
        super::common::body_to_string(app.oneshot(messages_post(&cred, json!("required"))).await?)
            .await?;

    assert_eq!(status.as_u16(), 400, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(v["error"]["type"], "invalid_request_error", "{body}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("tool_choice"),
        "the rejection must name the field the client got wrong: {body}"
    );

    assert!(
        h.upstream
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "a request the upstream API would refuse must not be forwarded to it"
    );
    Ok(())
}
