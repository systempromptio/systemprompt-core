//! `/consent` scope-consent endpoint — `handle_consent_get` and
//! `handle_consent_post`. The GET path renders the consent HTML page after
//! validating the requested scopes against the client's registered grant; the
//! POST path echoes the recorded decision. We drive the happy render, the
//! client-not-found, missing-scope, and invalid-scope error branches, plus
//! both allow/deny POST decisions.


use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, seed_oauth_client, test_db_pool,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, read_json, setup_ctx};

async fn consent_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    Ok(authenticated_router()
        .with_state(state)
        .layer(axum::Extension(ctx.config_arc().clone())))
}

async fn seeded_client() -> anyhow::Result<OAuthClientFixture> {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let user = UserId::new(format!("consent-owner-{}", Uuid::new_v4()));
    let p = pool.pool();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@consent.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    seed_oauth_client(&pool, &user).await
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("build")
}

#[tokio::test]
async fn consent_get_renders_page_for_registered_scopes() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let uri = format!(
        "/consent?client_id={}&scope=user%20anonymous&state=abc",
        client.client_id.as_str()
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains("Grant Access"),
        "missing consent header: {html}"
    );
    assert!(
        html.contains("Access your user data") && html.contains("Access your anonymous data"),
        "scope items not rendered: {html}"
    );
    Ok(())
}

#[tokio::test]
async fn consent_get_unknown_client_returns_invalid_client() -> anyhow::Result<()> {
    let app = consent_app().await?;
    let resp = app
        .oneshot(empty_get("/consent?client_id=no-such-client&scope=user"))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_get_missing_scope_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let uri = format!("/consent?client_id={}", client.client_id.as_str());
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_get_unregistered_scope_returns_invalid_scope() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let uri = format!(
        "/consent?client_id={}&scope=admin",
        client.client_id.as_str()
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_scope"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_post_allow_records_decision() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let body = serde_json::json!({
        "client_id": client.client_id.as_str(),
        "scope": "user anonymous",
        "state": "abc",
        "decision": "allow",
    });
    let resp = app.oneshot(json_post("/consent", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = read_json(resp).await?;
    assert_eq!(v["status"].as_str(), Some("processed"), "{v}");
    assert_eq!(v["decision"].as_str(), Some("allow"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_post_deny_records_decision() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let body = serde_json::json!({
        "client_id": client.client_id.as_str(),
        "decision": "deny",
    });
    let resp = app.oneshot(json_post("/consent", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = read_json(resp).await?;
    assert_eq!(v["decision"].as_str(), Some("deny"), "{v}");
    Ok(())
}
