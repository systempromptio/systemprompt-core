//! Full-router coverage for the Microsoft Teams inbound surface.
//!
//! Drives the real `teams_router` via `tower::ServiceExt::oneshot`. The
//! config-free edges (malformed body, unknown tenant, missing/bad bearer) need
//! no backend; the signed happy-path mints an RS256 activity token, serves the
//! Bot Connector `OpenID`/JWKS + token endpoints from a loopback wiremock (via
//! the `test-api` env overrides), runs the spawned dispatch, and asserts the
//! Adaptive Card reply reaches the Bot Connector.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use systemprompt_security::keys::RsaSigningKey;
use systemprompt_test_fixtures::{
    TEST_TEAMS_APP_ID, TEST_TEAMS_TENANT_ID, agent_reply_response_json, ensure_messaging_bootstrap,
    fixture_app_context, fixture_db_pool, install_test_signing_key, seed_agent_backend,
};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ISSUER: &str = "https://api.botframework.com";
const CONVERSATION_ID: &str = "conv-test-1";

fn router(ctx: &std::sync::Arc<systemprompt_runtime::AppContext>) -> axum::Router {
    systemprompt_api::routes::teams::teams_router().with_state((**ctx).clone())
}

fn activity_json(tenant: &str, service_url: &str) -> String {
    serde_json::json!({
        "type": "message",
        "id": "act-1",
        "serviceUrl": service_url,
        "text": "hello",
        "from": { "id": "29:user" },
        "conversation": { "id": CONVERSATION_ID, "tenantId": tenant },
    })
    .to_string()
}

fn post_messages(body: &str, bearer: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/messages")
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder
        .body(Body::from(body.to_owned()))
        .expect("request build")
}

#[tokio::test]
async fn malformed_activity_body_is_bad_request() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    let resp = router(&ctx)
        .oneshot(post_messages("not json", None))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn unknown_tenant_is_acked() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    let body = activity_json("tenant-unknown", "https://smba.example");
    let resp = router(&ctx).oneshot(post_messages(&body, None)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "unknown tenant is acked");
    Ok(())
}

#[tokio::test]
async fn missing_bearer_is_unauthorized() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    let body = activity_json(TEST_TEAMS_TENANT_ID, "https://smba.example");
    let resp = router(&ctx).oneshot(post_messages(&body, None)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn malformed_bearer_is_unauthorized() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    let body = activity_json(TEST_TEAMS_TENANT_ID, "https://smba.example");
    // `not-a-jwt` fails `decode_header` before any JWKS fetch.
    let resp = router(&ctx)
        .oneshot(post_messages(&body, Some("not-a-jwt")))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

fn mint(signing: &RsaSigningKey, service_url: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let claims = serde_json::json!({
        "iss": ISSUER,
        "aud": TEST_TEAMS_APP_ID,
        "exp": now + 3600,
        "serviceurl": service_url,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(signing.kid().to_owned());
    let der = signing.private_key().to_pkcs1_der().expect("der");
    let key = EncodingKey::from_rsa_der(der.as_bytes());
    encode(&header, &claims, &key).expect("mint")
}

fn jwk(signing: &RsaSigningKey) -> serde_json::Value {
    let pubkey = signing.public_key();
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pubkey.n().to_bytes_be());
    let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pubkey.e().to_bytes_be());
    serde_json::json!({ "kty": "RSA", "use": "sig", "alg": "RS256", "kid": signing.kid(), "n": n, "e": e })
}

#[tokio::test]
async fn signed_activity_dispatches_and_posts_the_card() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;

    // Agent backend the dispatch proxies to.
    let agent = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(agent_reply_response_json("teams reply text")),
        )
        .mount(&agent)
        .await;
    seed_agent_backend(&pool, &agent).await?;

    // Bot Connector: serves OpenID metadata, JWKS, the outbound token, and the
    // reply endpoint. `serviceUrl` points here.
    let connector = MockServer::start().await;
    let signing = RsaSigningKey::generate_bits(2048).expect("rsa");
    Mock::given(method("GET"))
        .and(path("/openid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jwks_uri": format!("{}/jwks", connector.uri()),
        })))
        .mount(&connector)
        .await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "keys": [jwk(&signing)] })),
        )
        .mount(&connector)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "bot-token",
            "expires_in": 3600,
        })))
        .mount(&connector)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/v3/conversations/{CONVERSATION_ID}/activities"
        )))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "id": "1" })))
        .expect(1)
        .mount(&connector)
        .await;

    // SAFETY: the test-api env overrides redirect the inbound JWKS fetch and
    // outbound token mint to the loopback connector. Under nextest each test is
    // its own process, so this does not leak into sibling tests.
    unsafe {
        std::env::set_var(
            "SYSTEMPROMPT_TEST_TEAMS_OPENID_URL",
            format!("{}/openid", connector.uri()),
        );
        std::env::set_var(
            "SYSTEMPROMPT_TEST_TEAMS_TOKEN_URL",
            format!("{}/token", connector.uri()),
        );
    }

    let token = mint(&signing, &connector.uri());
    let body = activity_json(TEST_TEAMS_TENANT_ID, &connector.uri());
    let resp = router(&ctx)
        .oneshot(post_messages(&body, Some(&token)))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "the route acks immediately");

    let posted = wait_for_activity(&connector).await;
    let card = String::from_utf8_lossy(&posted);
    assert!(
        card.contains("teams reply text"),
        "Adaptive Card carries the agent reply: {card}"
    );
    Ok(())
}

// The reply comes from a spawned task running the full dispatch pipeline
// (identity linking, authz, proxy round-trip); under a loaded shard that has
// been observed to stall past 30s, so the deadline must dwarf it.
async fn wait_for_activity(server: &MockServer) -> Vec<u8> {
    let suffix = format!("/v3/conversations/{CONVERSATION_ID}/activities");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(reqs) = server.received_requests().await
            && let Some(hit) = reqs.iter().find(|r| r.url.path() == suffix)
        {
            return hit.body.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "spawned reply never reached the Bot Connector within 120s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
