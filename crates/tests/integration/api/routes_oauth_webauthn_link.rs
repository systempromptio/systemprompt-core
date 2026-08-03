//! The passkey *link* ceremony and the authentication *finish* step.
//!
//! `start_link`, `finish_link`, `finish_auth` and `link_passkey_page` are never
//! entered by the existing suite. The pre-existing `/webauthn/link/start`
//! probe omits the mandatory `token` query parameter, so axum's `Query`
//! extractor rejects the request before the handler body runs — the assertion
//! `(200..600).contains(&status)` cannot fail, and the handler stayed at zero
//! calls. Supplying the parameter puts each handler on its real path; without a
//! live authenticator the ceremonies then fail at the service, which is the
//! branch that matters for a linking flow (a bad or replayed link token must
//! never attach a credential to an account).

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use axum::response::IntoResponse;
use systemprompt_api::routes::oauth::public_router;
use systemprompt_api::routes::oauth::webauthn::link::link_passkey_page;
use systemprompt_identifiers::ChallengeId;
use systemprompt_models::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{fixture_config, install_test_signing_key};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let mut config = fixture_config("postgres://x");
        config.api_external_url = "http://localhost".to_owned();
        let _ = Config::install(config);
    });
}

async fn app() -> anyhow::Result<Router> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(public_router().with_state(state))
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("build")
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("build")
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn credential() -> serde_json::Value {
    serde_json::json!({
        "id": "AAAA",
        "rawId": "AAAA",
        "type": "public-key",
        "response": {
            "attestationObject": "AAAA",
            "clientDataJSON": "AAAA"
        }
    })
}

#[tokio::test]
async fn link_start_with_a_blank_token_is_rejected_by_the_handler() -> anyhow::Result<()> {
    let resp = app()
        .await?
        .oneshot(get("/webauthn/link/start?token="))
        .await?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    assert!(
        v["error_description"]
            .as_str()
            .is_some_and(|d| d.contains("Token")),
        "the rejection must name the missing token: {v}"
    );
    Ok(())
}

#[tokio::test]
async fn link_start_with_an_unissued_token_mints_no_challenge() -> anyhow::Result<()> {
    let token = format!("link-{}", Uuid::new_v4().simple());

    let resp = app()
        .await?
        .oneshot(get(&format!("/webauthn/link/start?token={token}")))
        .await?;

    // A token the server never issued must not produce a registration
    // challenge — that challenge is what would attach a passkey to an account.
    assert_ne!(resp.status(), StatusCode::OK, "{}", resp.status());
    assert!(
        resp.headers().get("x-challenge-id").is_none(),
        "a rejected link attempt must not hand back a challenge id"
    );
    Ok(())
}

#[tokio::test]
async fn link_finish_with_an_unissued_token_attaches_nothing() -> anyhow::Result<()> {
    let resp = app()
        .await?
        .oneshot(json_post(
            "/webauthn/link/finish",
            serde_json::json!({
                "challenge_id": ChallengeId::new(format!("chal-{}", Uuid::new_v4().simple()))
                    .as_str(),
                "token": format!("link-{}", Uuid::new_v4().simple()),
                "credential": credential(),
            }),
        ))
        .await?;

    assert_ne!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert!(
        v["success"].as_bool() != Some(true),
        "an unissued link token must never report a successful link: {v}"
    );
    Ok(())
}

#[tokio::test]
async fn auth_finish_without_a_live_challenge_is_an_authentication_failure() -> anyhow::Result<()> {
    let resp = app()
        .await?
        .oneshot(json_post(
            "/webauthn/auth/finish",
            serde_json::json!({
                "challenge_id": ChallengeId::new(format!("chal-{}", Uuid::new_v4().simple()))
                    .as_str(),
                "credential": {
                    "id": "AAAA",
                    "rawId": "AAAA",
                    "type": "public-key",
                    "response": {
                        "authenticatorData": "AAAA",
                        "clientDataJSON": "AAAA",
                        "signature": "AAAA"
                    }
                }
            }),
        ))
        .await?;

    assert_ne!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert!(
        v["auth_token"].as_str().is_none(),
        "a failed ceremony must not mint a verified-authentication token: {v}"
    );
    Ok(())
}

#[tokio::test]
async fn the_link_page_renders_the_passkey_template() {
    let resp = link_passkey_page().await.into_response();

    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.contains("text/html"),
        "the link page is served as HTML, got {content_type}"
    );
    let body = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("the template is small and fully buffered");
    assert!(!body.is_empty(), "the link page must not be blank");
}

#[tokio::test]
async fn the_assembled_oauth_router_serves_both_halves() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );

    // `router()` is the merge of the public and authenticated halves; nothing
    // in the suite builds it, so a route lost from the merge would go unnoticed.
    let merged = systemprompt_api::routes::oauth::router().with_state(state);
    let resp = merged.oneshot(get("/health")).await?;

    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}
