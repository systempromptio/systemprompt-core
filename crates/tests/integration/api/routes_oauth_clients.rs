//! OAuth dynamic client management: the admin CRUD surface under `/clients`
//! (`create`, `list`, `get`, `update`, `delete`) and the RFC 7592
//! client-configuration surface under `/register/{client_id}`.
//!
//! The `/clients` handlers read the authenticated `RequestContext` for the
//! owning user, so the router is layered with a context-injection middleware
//! (mirroring production's route-mount middleware). The `/register` handlers
//! authenticate with a registration access token — validation only checks the
//! `reg_` prefix, so the tests exercise the missing/mis-scheme/wrong-prefix
//! rejections plus the found/not-found branches.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::Config;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::OAuthState;
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
        let _ = Config::install(fixture_config("postgres://x"));
    });
}

fn ctx_for(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("oauth-clients"),
        ContextId::generate(),
        AgentName::system(),
    )
    .with_actor(Actor::user(user.clone()))
}

async fn clients_app(user: UserId) -> anyhow::Result<Router> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    let inject = move |mut req: Request<Body>, next: Next| {
        let ctx = ctx_for(&user);
        async move {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
    };
    Ok(authenticated_router()
        .layer(middleware::from_fn(inject))
        .with_state(state))
}

async fn seed_owner() -> anyhow::Result<UserId> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@clients.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    Ok(user)
}

async fn seed_existing_client() -> anyhow::Result<(UserId, OAuthClientFixture)> {
    let owner = seed_owner().await?;
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let client = seed_oauth_client(&pool, &owner).await?;
    Ok((owner, client))
}

fn json_request(method: http::Method, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("build")
}

fn get_with_auth(uri: &str, auth: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(http::Method::GET).uri(uri);
    if let Some(value) = auth {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    builder.body(Body::empty()).expect("build")
}

fn body_with_auth(
    method: http::Method,
    uri: &str,
    auth: Option<&str>,
    body: Option<serde_json::Value>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(value) = auth {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    match body {
        Some(json) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            builder.body(Body::from(json.to_string())).expect("build")
        },
        None => builder.body(Body::empty()).expect("build"),
    }
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn create_client_returns_201_with_secret() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let client_id = format!("created-{}", Uuid::new_v4().simple());
    let body = serde_json::json!({
        "client_id": client_id,
        "name": "created client",
        "redirect_uris": ["https://app.example/callback"],
        "scopes": ["openid", "profile"],
    });
    let resp = app
        .oneshot(json_request(http::Method::POST, "/clients", body))
        .await?;
    assert_eq!(resp.status(), StatusCode::CREATED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["client_id"].as_str(), Some(client_id.as_str()), "{v}");
    assert!(
        v["client_secret"].as_str().is_some_and(|s| !s.is_empty()),
        "create must surface the generated secret exactly once: {v}"
    );
    Ok(())
}

#[tokio::test]
async fn create_client_duplicate_id_returns_409() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let client_id = format!("dup-{}", Uuid::new_v4().simple());
    let body = || {
        serde_json::json!({
            "client_id": client_id,
            "name": "dup client",
            "redirect_uris": ["https://app.example/callback"],
            "scopes": ["openid"],
        })
    };
    let first = app
        .clone()
        .oneshot(json_request(http::Method::POST, "/clients", body()))
        .await?;
    assert_eq!(first.status(), StatusCode::CREATED, "{}", first.status());
    let second = app
        .oneshot(json_request(http::Method::POST, "/clients", body()))
        .await?;
    assert_eq!(second.status(), StatusCode::CONFLICT, "{}", second.status());
    Ok(())
}

#[tokio::test]
async fn list_clients_returns_data_envelope() -> anyhow::Result<()> {
    let (owner, _client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app.oneshot(get_with_auth("/clients", None)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert!(v["data"].is_array(), "{v}");
    assert!(v["meta"]["pagination"].is_object(), "{v}");
    Ok(())
}

#[tokio::test]
async fn get_client_returns_client() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(get_with_auth(
            &format!("/clients/{}", client.client_id.as_str()),
            None,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(
        v["data"]["client_id"].as_str(),
        Some(client.client_id.as_str()),
        "{v}"
    );
    Ok(())
}

#[tokio::test]
async fn get_unknown_client_returns_404() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(get_with_auth("/clients/no-such-client", None))
        .await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn update_client_applies_new_name() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let body = serde_json::json!({
        "name": "renamed client",
        "redirect_uris": ["https://renamed.example/callback"],
        "scopes": ["openid", "profile"],
    });
    let resp = app
        .oneshot(json_request(
            http::Method::PUT,
            &format!("/clients/{}", client.client_id.as_str()),
            body,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["data"]["name"].as_str(), Some("renamed client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn update_unknown_client_returns_404() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let body = serde_json::json!({ "name": "x" });
    let resp = app
        .oneshot(json_request(
            http::Method::PUT,
            "/clients/no-such-client",
            body,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn delete_client_returns_204() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(body_with_auth(
            http::Method::DELETE,
            &format!("/clients/{}", client.client_id.as_str()),
            None,
            None,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn delete_unknown_client_returns_404() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(body_with_auth(
            http::Method::DELETE,
            "/clients/no-such-client",
            None,
            None,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{}", resp.status());
    Ok(())
}

const REG_TOKEN: &str = "reg_test-registration-access-token";

#[tokio::test]
async fn client_config_get_without_auth_returns_401() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(get_with_auth(
            &format!("/register/{}", client.client_id.as_str()),
            None,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn client_config_get_non_reg_prefix_returns_401() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(get_with_auth(
            &format!("/register/{}", client.client_id.as_str()),
            Some("Bearer not-a-reg-token"),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn client_config_get_returns_registration() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(get_with_auth(
            &format!("/register/{}", client.client_id.as_str()),
            Some(&format!("Bearer {REG_TOKEN}")),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(
        v["client_id"].as_str(),
        Some(client.client_id.as_str()),
        "{v}"
    );
    assert_eq!(v["client_secret"].as_str(), Some("***REDACTED***"), "{v}");
    assert_eq!(
        v["registration_access_token"].as_str(),
        Some(REG_TOKEN),
        "{v}"
    );
    Ok(())
}

#[tokio::test]
async fn client_config_get_unknown_client_returns_400() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(get_with_auth(
            "/register/no-such-client",
            Some(&format!("Bearer {REG_TOKEN}")),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn client_config_update_returns_new_metadata() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let body = serde_json::json!({
        "client_name": "config-updated",
        "redirect_uris": ["https://cfg.example/callback"],
    });
    let resp = app
        .oneshot(body_with_auth(
            http::Method::PUT,
            &format!("/register/{}", client.client_id.as_str()),
            Some(&format!("Bearer {REG_TOKEN}")),
            Some(body),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["client_name"].as_str(), Some("config-updated"), "{v}");
    Ok(())
}

#[tokio::test]
async fn client_config_update_without_auth_returns_401() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let body = serde_json::json!({ "client_name": "x" });
    let resp = app
        .oneshot(body_with_auth(
            http::Method::PUT,
            &format!("/register/{}", client.client_id.as_str()),
            None,
            Some(body),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn client_config_delete_returns_204() -> anyhow::Result<()> {
    let (owner, client) = seed_existing_client().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(body_with_auth(
            http::Method::DELETE,
            &format!("/register/{}", client.client_id.as_str()),
            Some(&format!("Bearer {REG_TOKEN}")),
            None,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn client_config_delete_unknown_client_returns_400() -> anyhow::Result<()> {
    let owner = seed_owner().await?;
    let app = clients_app(owner).await?;
    let resp = app
        .oneshot(body_with_auth(
            http::Method::DELETE,
            "/register/no-such-client",
            Some(&format!("Bearer {REG_TOKEN}")),
            None,
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    Ok(())
}
