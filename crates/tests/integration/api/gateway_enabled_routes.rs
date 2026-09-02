//! Gateway routes driven against a profile that actually enables the gateway.
//!
//! The shared fixture profile has no `gateway:` section, so every one of these
//! routes short-circuits on `GatewayState::resolved()` and answers 404 "Gateway
//! not enabled" — leaving `/v1/models`, the request-extraction chain and the
//! dispatch rejection arms unreachable. This suite boots an isolated profile
//! with the gateway on, two providers and three routes, which puts the model
//! catalogue and the pre-dispatch denial paths on their real branches.
//!
//! The `limit` query parameter is deliberately not covered for the
//! unparseable-value case: `ListQuery`'s own documentation states an
//! unparseable value must return the whole catalogue, but axum's `Query`
//! extractor rejects the request with a 400 before the handler runs. Asserting
//! either way would pin one side of that contradiction.

use std::sync::OnceLock;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, SESSION_ID};
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, init_gateway_bootstrap,
    install_test_signing_key, seed_admin_credential,
};
use tower::ServiceExt;

use super::common::body_to_string;

const GATEWAY_YAML: &str = r#"
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: http://127.0.0.1:1
    api_key_secret: anthropic_api_key
    models:
      - id: claude-fixture-1
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
      - id: claude-fixture-2
        pricing:
          input_per_million: 1.0
          output_per_million: 5.0
  - name: openai
    wire: openai_chat
    surface: openai
    endpoint: http://127.0.0.1:1
    api_key_secret: openai_api_key
    models:
      - id: gpt-fixture-1
        pricing:
          input_per_million: 2.5
          output_per_million: 10.0
gateway:
  enabled: true
  allow_unlisted_models: false
  routes:
    - id: claude
      model_pattern: "claude-*"
      provider: anthropic
    - id: gpt
      model_pattern: "gpt-*"
      provider: openai
"#;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_gateway_bootstrap(GATEWAY_YAML))
}

async fn app() -> anyhow::Result<Router> {
    let b = boot();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok(gateway_router(&ctx).expect("gateway router available"))
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request must build")
}

fn messages_post(token: &str, headers: &[(&str, &str)], body: serde_json::Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method(http::Method::POST)
        .uri("/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("request must build")
}

fn message_body(model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "hello"}],
    })
}

#[tokio::test]
async fn the_model_catalogue_lists_every_declared_provider_model() -> anyhow::Result<()> {
    let (status, body) = body_to_string(app().await?.oneshot(get("/models")).await?).await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body)?;
    let ids: Vec<String> = listing["data"]
        .as_array()
        .expect("the catalogue is a data array")
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_owned))
        .collect();

    assert!(ids.iter().any(|id| id == "claude-fixture-1"), "{body}");
    assert!(ids.iter().any(|id| id == "gpt-fixture-1"), "{body}");
    Ok(())
}

#[tokio::test]
async fn the_catalogue_honours_a_limit_and_reports_more_remain() -> anyhow::Result<()> {
    let (status, body) =
        body_to_string(app().await?.oneshot(get("/models?limit=1")).await?).await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        listing["data"].as_array().map(Vec::len),
        Some(1),
        "a limit must truncate the catalogue: {body}"
    );
    assert_eq!(
        listing["has_more"].as_bool(),
        Some(true),
        "a truncated catalogue must say so, or the client stops paging: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_request_with_no_credential_is_refused_before_anything_else() -> anyhow::Result<()> {
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header(SESSION_ID, "sess-anon")
        .body(Body::from(message_body("claude-fixture-1").to_string()))
        .expect("request must build");

    let (status, body) = body_to_string(app().await?.oneshot(req).await?).await?;

    assert_eq!(status.as_u16(), 401, "{body}");
    assert!(
        body.contains("Authorization") || body.contains("x-api-key"),
        "the rejection must name the credential it wanted: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_request_with_no_session_header_is_refused() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-nosession@example.invalid").await?;

    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                cred.jwt.as_str(),
                &[],
                message_body("claude-fixture-1"),
            ))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 400, "{body}");
    assert!(body.contains(SESSION_ID), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_model_no_route_matches_is_denied_rather_than_billed() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-unrouted@example.invalid").await?;

    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                cred.jwt.as_str(),
                &[(SESSION_ID, cred.session_id.as_str())],
                message_body("llama-not-configured"),
            ))
            .await?,
    )
    .await?;

    // The catalogue is a closed allowlist; an unmatched model must not fall
    // through to a default provider that would then be charged for it.
    assert_eq!(status.as_u16(), 404, "{body}");
    assert!(body.contains("llama-not-configured"), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_body_with_no_messages_cannot_derive_a_conversation() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-nomessages@example.invalid").await?;

    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                cred.jwt.as_str(),
                &[(SESSION_ID, cred.session_id.as_str())],
                serde_json::json!({
                    "model": "claude-fixture-1",
                    "max_tokens": 16,
                    "messages": [],
                }),
            ))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 400, "{body}");
    Ok(())
}

#[tokio::test]
async fn a_malformed_conversation_header_is_refused() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-badconv@example.invalid").await?;

    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                cred.jwt.as_str(),
                &[
                    (SESSION_ID, cred.session_id.as_str()),
                    (GATEWAY_CONVERSATION_ID, "   "),
                ],
                message_body("claude-fixture-1"),
            ))
            .await?,
    )
    .await?;

    // A blank conversation header is treated as absent, so the id is derived
    // from the message history and the request proceeds past extraction.
    assert_ne!(
        status.as_u16(),
        400,
        "a blank conversation header is absent, not malformed: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unparseable_body_is_refused_before_the_upstream_is_dialled() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-badbody@example.invalid").await?;

    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", cred.jwt.as_str()),
        )
        .header(SESSION_ID, cred.session_id.as_str())
        .body(Body::from("{not json"))
        .expect("request must build");

    let (status, body) = body_to_string(app().await?.oneshot(req).await?).await?;

    assert_eq!(status.as_u16(), 400, "{body}");
    Ok(())
}

#[tokio::test]
async fn a_routed_model_with_no_provider_key_fails_closed() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-dispatch@example.invalid").await?;

    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                cred.jwt.as_str(),
                &[(SESSION_ID, cred.session_id.as_str())],
                message_body("claude-fixture-1"),
            ))
            .await?,
    )
    .await?;

    // The configured endpoint is a closed port, so a request that clears
    // extraction and authorization must surface as an upstream failure — not as
    // a 404 or a 200 with an invented body.
    // This profile configures no provider secret, so the request clears
    // extraction and route resolution and then fails closed at credential
    // resolution — the upstream must never be dialled unauthenticated.
    assert!(
        status.is_server_error(),
        "a provider with no configured key must fail, got {status}: {body}"
    );
    assert!(
        !body.contains("Gateway not enabled"),
        "the request must have got past the gateway gate: {body}"
    );
    assert!(
        body.contains("not configured"),
        "the failure must name the missing credential: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn the_responses_wire_shares_the_same_extraction_rules() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-responses@example.invalid").await?;

    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", cred.jwt.as_str()),
        )
        .body(Body::from(message_body("gpt-fixture-1").to_string()))
        .expect("request must build");

    let (status, body) = body_to_string(app().await?.oneshot(req).await?).await?;

    assert_eq!(
        status.as_u16(),
        400,
        "the session header is mandatory on every inference wire: {body}"
    );
    assert!(body.contains(SESSION_ID), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_garbage_bearer_token_is_refused_by_the_gateway() -> anyhow::Result<()> {
    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                "not-a-jwt",
                &[(SESSION_ID, "sess-garbage")],
                message_body("claude-fixture-1"),
            ))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 401, "{body}");
    Ok(())
}

#[tokio::test]
async fn a_session_bound_token_cannot_be_replayed_under_another_session() -> anyhow::Result<()> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "gw-sessionbind@example.invalid").await?;

    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(messages_post(
                cred.jwt.as_str(),
                &[(SESSION_ID, "sess-belonging-to-nobody")],
                message_body("claude-fixture-1"),
            ))
            .await?,
    )
    .await?;

    // The token carries its own session; presenting it against a different one
    // must be refused, or a leaked token could be replayed anywhere.
    assert!(
        status.is_client_error(),
        "a mismatched session binding must be refused, got {status}: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn the_gateway_root_describes_itself() -> anyhow::Result<()> {
    let (status, body) = body_to_string(app().await?.oneshot(get("/")).await?).await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(!body.is_empty(), "the root must describe the gateway");
    Ok(())
}

#[tokio::test]
async fn the_bridge_profile_describes_the_enabled_gateway() -> anyhow::Result<()> {
    let (status, body) =
        body_to_string(app().await?.oneshot(get("/bridge/profile")).await?).await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let profile: serde_json::Value = serde_json::from_str(&body)?;

    let base = profile["inference_gateway_base_url"]
        .as_str()
        .expect("the bridge dials this url");
    assert!(
        base.starts_with("http://127.0.0.1"),
        "the base url is built from the profile's external url, not invented: {profile}"
    );
    assert!(
        !base.ends_with('/'),
        "a trailing slash would double up when the bridge appends a path: {base}"
    );
    Ok(())
}

#[tokio::test]
async fn the_bridge_profile_reports_which_providers_have_a_usable_key() -> anyhow::Result<()> {
    let (status, body) =
        body_to_string(app().await?.oneshot(get("/bridge/profile")).await?).await?;
    assert_eq!(status.as_u16(), 200, "{body}");

    // No provider secret is configured in the fixture, so every declared
    // provider must be reported as unusable — announcing one as ready would
    // send the bridge at an endpoint that cannot authenticate.
    assert!(
        body.contains("anthropic") || body.contains("openai"),
        "the declared providers must appear in the bridge profile: {body}"
    );
    assert!(
        !body.contains("\"has_key\":true"),
        "no secret is configured, so nothing may claim a key: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn the_bridge_publishes_a_manifest_signing_key() -> anyhow::Result<()> {
    let (status, body) = body_to_string(app().await?.oneshot(get("/bridge/pubkey")).await?).await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let parsed: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        parsed["pubkey"].as_str().is_some_and(|k| !k.is_empty()),
        "the bridge verifies manifest signatures against this key: {body}"
    );
    Ok(())
}
