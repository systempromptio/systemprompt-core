//! WebAuthn passkey ceremonies: the registration/authentication *start*
//! handlers (which generate a server-side challenge with no authenticator
//! present), the registration *finish* validation arms, and the
//! `/webauthn/complete` bridge.
//!
//! `start_register` succeeds because the fixture `Config` carries a valid
//! `api_external_url` from which the relying-party id is derived; the input
//! validators reject empty/oversized/ill-formed usernames and bad emails before
//! any challenge is minted. `/webauthn/complete` is driven end to end by
//! pre-seeding a verified-authentication token directly into the process-wide
//! `WebAuthnRegistry` singleton — the same instance the handler resolves — so
//! no live ceremony is required.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::UserId;
use systemprompt_models::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::generate_secure_token;
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_config, fixture_db_pool,
    install_test_signing_key, seed_oauth_client,
};
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

async fn webauthn_app() -> anyhow::Result<Router> {
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

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn empty_post(uri: &str) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
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

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method(http::Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("build")
}

#[tokio::test]
async fn register_start_generates_challenge() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let username = format!("user{}", Uuid::new_v4().simple());
    let email = format!("{username}@webauthn.invalid");
    let resp = app
        .oneshot(empty_post(&format!(
            "/webauthn/register/start?username={username}&email={email}"
        )))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    assert!(
        resp.headers().get("x-challenge-id").is_some(),
        "start must return a challenge id header"
    );
    let v = read_json(resp).await?;
    assert!(v["publicKey"]["challenge"].is_string(), "{v}");
    Ok(())
}

#[tokio::test]
async fn register_start_empty_username_returns_invalid_request() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_post(
            "/webauthn/register/start?username=&email=a@b.com",
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn register_start_invalid_email_returns_invalid_request() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_post(
            "/webauthn/register/start?username=validname&email=not-an-email",
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn register_start_illegal_username_chars_returns_invalid_request() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_post(
            "/webauthn/register/start?username=bad%20name%21&email=a@b.com",
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn register_finish_empty_username_returns_invalid_request() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let body = serde_json::json!({
        "challenge_id": "some-challenge",
        "username": "",
        "email": "a@b.com",
        "credential": {
            "id": "AAAA",
            "rawId": "AAAA",
            "type": "public-key",
            "response": {
                "attestationObject": "AAAA",
                "clientDataJSON": "AAAA"
            }
        }
    });
    let resp = app
        .oneshot(json_post("/webauthn/register/finish", body))
        .await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn authenticate_start_unknown_email_is_client_error() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let email = format!("nobody-{}@webauthn.invalid", Uuid::new_v4().simple());
    let resp = app
        .oneshot(empty_post(&format!("/webauthn/auth/start?email={email}")))
        .await?;
    assert!(
        resp.status().is_client_error() || resp.status().is_server_error(),
        "unknown email must not succeed, got {}",
        resp.status()
    );
    Ok(())
}

async fn seed_user_and_client() -> anyhow::Result<(UserId, OAuthClientFixture)> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@webauthn.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    let client = seed_oauth_client(&pool, &user).await?;
    Ok((user, client))
}

async fn inject_verified_auth(user: &UserId) -> anyhow::Result<String> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let repo = OAuthRepository::new(&pool).map_err(|e| anyhow::anyhow!("oauth repo: {e}"))?;
    let (_pool, ctx) = setup_ctx().await?;
    let service = WebAuthnRegistry::get_or_create_service(repo, ctx.user_provider().expect("user"))
        .await
        .map_err(|e| anyhow::anyhow!("webauthn service: {e}"))?;
    let token = generate_secure_token("webauthn_verified");
    service
        .store_verified_authentication(token.clone(), user.clone())
        .await;
    Ok(token)
}

#[tokio::test]
async fn webauthn_complete_success_issues_authorization_code() -> anyhow::Result<()> {
    ensure_config();
    install_test_signing_key();
    let (user, client) = seed_user_and_client().await?;
    let token = inject_verified_auth(&user).await?;
    let app = webauthn_app().await?;
    let uri = format!(
        "/webauthn/complete?user_id={}&auth_token={}&client_id={}&redirect_uri={}&scope=user",
        user.as_str(),
        token,
        client.client_id.as_str(),
        "http%3A%2F%2F127.0.0.1%2Fcallback",
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert!(
        v["authorization_code"]
            .as_str()
            .is_some_and(|c| !c.is_empty()),
        "complete must mint an authorization code: {v}"
    );
    assert_eq!(
        v["client_id"].as_str(),
        Some(client.client_id.as_str()),
        "{v}"
    );
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_user_mismatch_returns_access_denied() -> anyhow::Result<()> {
    ensure_config();
    install_test_signing_key();
    let (user, client) = seed_user_and_client().await?;
    let token = inject_verified_auth(&user).await?;
    let app = webauthn_app().await?;
    let other_user = UserId::new(Uuid::new_v4().to_string());
    let uri = format!(
        "/webauthn/complete?user_id={}&auth_token={}&client_id={}&redirect_uri={}",
        other_user.as_str(),
        token,
        client.client_id.as_str(),
        "http%3A%2F%2F127.0.0.1%2Fcallback",
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("access_denied"), "{v}");
    Ok(())
}
