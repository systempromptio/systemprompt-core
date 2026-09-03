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
use systemprompt_identifiers::{ChallengeId, UserId};
use systemprompt_models::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{CreateSetupTokenParams, SetupTokenPurpose};
use systemprompt_oauth::services::webauthn::hash_token;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, fixture_db_pool, install_test_signing_key, seed_user_row,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softtoken::SoftToken;
use webauthn_rs::prelude::CreationChallengeResponse;

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
        ctx.oauth_repositories().oauth.clone(),
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
        ctx.oauth_repositories().oauth.clone(),
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

// A double-fired `/link/start` must describe one ceremony: the browser
// completes `create()` for the first response and the client may send either
// challenge id back, so both must be accepted by `/link/finish`.

async fn issue_link_token(ctx: &systemprompt_runtime::AppContext) -> (UserId, String) {
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{}@link.invalid", Uuid::new_v4().simple());
    let pool = fixture_db_pool(&ensure_test_bootstrap().database_url)
        .await
        .expect("pool");
    seed_user_row(&pool, &user_id, &email)
        .await
        .expect("seed user");
    let raw = format!("link-{}", Uuid::new_v4().simple());
    ctx.oauth_repositories()
        .oauth
        .store_setup_token(CreateSetupTokenParams {
            user_id: user_id.clone(),
            token_hash: hash_token(&raw),
            purpose: SetupTokenPurpose::CredentialLink,
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(600),
        })
        .await
        .expect("store setup token");
    (user_id, raw)
}

async fn linked_app() -> anyhow::Result<(Router, Arc<systemprompt_runtime::AppContext>)> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok((public_router().with_state(state), ctx))
}

async fn start_link(
    app: &Router,
    token: &str,
) -> anyhow::Result<(String, CreationChallengeResponse)> {
    let resp = app
        .clone()
        .oneshot(get(&format!("/webauthn/link/start?token={token}")))
        .await?;
    let status = resp.status();
    let challenge_id = resp
        .headers()
        .get("x-challenge-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = read_json(resp).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let challenge_id = challenge_id.expect("x-challenge-id header");
    let ccr: CreationChallengeResponse = serde_json::from_value(body["challenge"].clone())?;
    Ok((challenge_id, ccr))
}

fn softtoken() -> WebauthnAuthenticator<SoftToken> {
    let (token, _ca) = SoftToken::new(true).expect("softtoken");
    WebauthnAuthenticator::new(token)
}

fn rp_origin() -> url::Url {
    url::Url::parse("http://localhost").expect("origin")
}

#[tokio::test]
async fn two_link_starts_return_one_challenge_id() -> anyhow::Result<()> {
    let (app, ctx) = linked_app().await?;
    let (_user, token) = issue_link_token(&ctx).await;

    let (first_id, first) = start_link(&app, &token).await?;
    let (second_id, second) = start_link(&app, &token).await?;

    assert_eq!(
        second_id, first_id,
        "overlapping starts must share one challenge id"
    );
    assert_eq!(
        second.public_key.challenge, first.public_key.challenge,
        "overlapping starts must carry the same challenge bytes"
    );
    Ok(())
}

#[tokio::test]
async fn link_finish_accepts_the_id_from_either_start() -> anyhow::Result<()> {
    let (app, ctx) = linked_app().await?;
    let (user_id, token) = issue_link_token(&ctx).await;
    let mut auth = softtoken();

    let (_first_id, first) = start_link(&app, &token).await?;
    let (second_id, _second) = start_link(&app, &token).await?;
    let cred = auth
        .do_registration(rp_origin(), first)
        .expect("registration against the first response");

    let resp = app
        .clone()
        .oneshot(json_post(
            "/webauthn/link/finish",
            serde_json::json!({
                "challenge_id": second_id,
                "token": token,
                "credential": serde_json::to_value(&cred)?,
            }),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["success"].as_bool(), Some(true), "{v}");
    assert_eq!(v["user_id"].as_str(), Some(user_id.as_str()), "{v}");

    let creds = ctx
        .oauth_repositories()
        .oauth
        .list_webauthn_credentials(&user_id)
        .await?;
    assert_eq!(
        creds.len(),
        1,
        "the credential the browser holds is on the server"
    );

    let resp = app
        .oneshot(get(&format!("/webauthn/link/start?token={token}")))
        .await?;
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "a linked token must not start again"
    );
    Ok(())
}

#[tokio::test]
async fn link_finish_with_a_superseded_challenge_is_rejected() -> anyhow::Result<()> {
    let (app, ctx) = linked_app().await?;
    let (user_id, token_a) = issue_link_token(&ctx).await;
    let raw_b = format!("link-{}", Uuid::new_v4().simple());
    ctx.oauth_repositories()
        .oauth
        .store_setup_token(CreateSetupTokenParams {
            user_id: user_id.clone(),
            token_hash: hash_token(&raw_b),
            purpose: SetupTokenPurpose::CredentialLink,
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(600),
        })
        .await?;
    let mut auth = softtoken();

    let (id_a, ccr_a) = start_link(&app, &token_a).await?;
    let (id_b, _ccr_b) = start_link(&app, &raw_b).await?;
    assert_ne!(id_b, id_a, "a second token supersedes the first ceremony");

    let cred_a = auth
        .do_registration(rp_origin(), ccr_a)
        .expect("registration A");
    let resp = app
        .oneshot(json_post(
            "/webauthn/link/finish",
            serde_json::json!({
                "challenge_id": id_a,
                "token": token_a,
                "credential": serde_json::to_value(&cred_a)?,
            }),
        ))
        .await?;
    assert_ne!(resp.status(), StatusCode::OK, "{}", resp.status());

    let creds = ctx
        .oauth_repositories()
        .oauth
        .list_webauthn_credentials(&user_id)
        .await?;
    assert!(
        creds.is_empty(),
        "a superseded ceremony must attach nothing"
    );
    Ok(())
}
