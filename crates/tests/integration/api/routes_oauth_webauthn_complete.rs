//! `/webauthn/complete` — `handle_webauthn_complete`. This bridges a verified
//! WebAuthn authentication token into an authorization code. Without a real
//! verified-authentication token in the WebAuthn registry, every request
//! resolves to a deterministic error: a missing `auth_token` is
//! `invalid_request`, and an unverifiable token is `access_denied`. We drive
//! the query-parameter validation and error mappings; the success path
//! requires a live WebAuthn ceremony (flagged).
//!
//! This file's config installs `allow_registration: false`, so it also hosts
//! the disabled-registration 403 gate tests for `register/start|finish`.


use axum::Router;
use axum::http::StatusCode;
use systemprompt_api::routes::oauth::public_router;
use systemprompt_oauth::OAuthState;
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

use super::common::{empty_get, json_post, read_json, setup_ctx};

async fn webauthn_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_signing_keys(),
    );
    Ok(public_router(std::sync::Arc::new(
        systemprompt_oauth_issuance::JwksCache::new(),
    ))
    .with_state(state)
    .layer(axum::Extension(ctx.config_arc().clone())))
}

#[tokio::test]
async fn webauthn_complete_missing_user_id_returns_4xx() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    // `user_id` is a required query field; the Query extractor rejects the
    // request before the handler runs.
    let resp = app.oneshot(empty_get("/webauthn/complete")).await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_missing_auth_token_returns_invalid_request() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_get("/webauthn/complete?user_id=some-user"))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(
        v["error"].as_str(),
        Some("invalid_request"),
        "missing auth_token must be invalid_request, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_unverifiable_token_is_error() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/webauthn/complete?user_id=some-user&auth_token=not-a-real-token",
        ))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    // An unconsumable auth token never yields a success; the fixture context
    // has no usable WebAuthn relying-party config, so the route surfaces an
    // OAuth-shaped error (access_denied, or server_error if init fails first).
    assert!(
        status.is_client_error() || status.is_server_error(),
        "{status} {v}"
    );
    assert!(
        v.get("error").and_then(|e| e.as_str()).is_some(),
        "missing error field: {v}"
    );
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_with_full_query_runs_handler() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/webauthn/complete?user_id=u&auth_token=tok&client_id=c&redirect_uri=http%3A%2F%2Flocalhost%2Fcb&scope=openid&state=st&code_challenge=ch&code_challenge_method=S256",
        ))
        .await?;
    let status = resp.status().as_u16();
    // The token is still unverifiable, so this lands on access_denied; the
    // point is to drive query deserialisation of every optional field.
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn register_start_is_forbidden_while_registration_is_disabled() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(json_post(
            "/webauthn/register/start?username=validname&email=a@b.com",
            serde_json::json!({}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("access_denied"), "{v}");
    assert_eq!(
        v["error_description"].as_str(),
        Some("registration_disabled"),
        "{v}"
    );
    Ok(())
}

#[tokio::test]
async fn register_finish_is_forbidden_while_registration_is_disabled() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let body = serde_json::json!({
        "challenge_id": "chal",
        "username": "validname",
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
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let v = read_json(resp).await?;
    assert_eq!(
        v["error_description"].as_str(),
        Some("registration_disabled"),
        "{v}"
    );
    Ok(())
}
