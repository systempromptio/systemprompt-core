//! End-to-end proxy forwarding through a stub backend.
//!
//! Stands up a `wiremock` MockServer as the local backend, registers a
//! `services` row (status `running`, port = the mock's port), and drives a
//! request through the bare `proxy::agents` / `proxy::mcp` routers so the full
//! forwarding pipeline runs: `ServiceResolver` (DB lookup + running check),
//! `AccessValidator` (OAuth requirement lookup + bearer validation), backend
//! URL building, request-context header injection, the outbound `reqwest`
//! send, and `ResponseHandler` re-assembly of the upstream response.
//!
//! The bare routers carry no middleware, so each request is given a
//! `RequestContext` extension manually (the proxy refuses to forward without
//! one). A self-issued admin JWT — minted with the live `Config` issuer and the
//! process-wide test signing key — satisfies the bearer check for any service
//! name (`validate_service_access` accepts the standard audience set).

use std::sync::Once;

use axum::body::Body;
use axum::http::{header, Request};
use axum::middleware::{self, Next};
use axum::response::Response;
use systemprompt_api::routes::proxy::{agents, mcp};
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::Config;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, install_test_signing_key, mint_admin_jwt,
};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{body_to_string, setup_ctx};

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let b = ensure_test_bootstrap();
        let _ = Config::install(fixture_config(&b.database_url));
    });
}

fn ctx_token() -> String {
    ensure_config();
    install_test_signing_key();
    let issuer = Config::get().expect("config installed").jwt_issuer.clone();
    let uid = UserId::new(Uuid::new_v4().to_string());
    mint_admin_jwt(&uid, "proxy-fwd@test.invalid", &issuer)
        .as_str()
        .to_owned()
}

async fn inject_ctx(
    axum::extract::State(token): axum::extract::State<String>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let rc = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::system(),
    )
    .with_auth_token(token);
    req.extensions_mut().insert(rc);
    next.run(req).await
}

async fn register_running_service(
    pool: &systemprompt_database::DbPool,
    name: &str,
    module: &str,
    port: u16,
) -> anyhow::Result<()> {
    let repo = ServiceRepository::new(pool)?;
    repo.create_service(CreateServiceInput {
        name,
        module_name: module,
        status: "running",
        port,
        binary_mtime: None,
    })
    .await?;
    Ok(())
}

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

fn authed_post(uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .expect("request build")
}

#[tokio::test]
async fn agent_proxy_forwards_get_to_backend() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("backend-ok"))
        .mount(&backend)
        .await;

    let name = unique_name("custom-fwd");
    register_running_service(&pool, &name, "custom", backend.address().port()).await?;

    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app.oneshot(authed_get(&format!("/{name}"), &token)).await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("backend-ok"), "{body}");
    Ok(())
}

#[tokio::test]
async fn agent_proxy_forwards_subpath_with_query() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/items"))
        .respond_with(ResponseTemplate::new(200).set_body_string("items-list"))
        .mount(&backend)
        .await;

    let name = unique_name("custom-sub");
    register_running_service(&pool, &name, "custom", backend.address().port()).await?;

    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_get(&format!("/{name}/api/v1/items?limit=5"), &token))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("items-list"), "{body}");
    Ok(())
}

#[tokio::test]
async fn agent_proxy_forwards_post_body() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(ResponseTemplate::new(201).set_body_string("created"))
        .mount(&backend)
        .await;

    let name = unique_name("custom-post");
    register_running_service(&pool, &name, "custom", backend.address().port()).await?;

    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_post(
            &format!("/{name}/submit"),
            &token,
            r#"{"k":"v"}"#,
        ))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 201, "{body}");
    assert!(body.contains("created"), "{body}");
    Ok(())
}

#[tokio::test]
async fn agent_proxy_propagates_backend_error_status() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boom"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .mount(&backend)
        .await;

    let name = unique_name("custom-err");
    register_running_service(&pool, &name, "custom", backend.address().port()).await?;

    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_get(&format!("/{name}/boom"), &token))
        .await?;
    assert_eq!(resp.status().as_u16(), 503);
    Ok(())
}

#[tokio::test]
async fn agent_proxy_running_service_without_context_is_error() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&backend)
        .await;

    let name = unique_name("custom-noctx");
    register_running_service(&pool, &name, "custom", backend.address().port()).await?;

    let token = ctx_token();
    // No inject_ctx middleware — the proxy must refuse without a RequestContext.
    let app = agents::router(&ctx);
    let resp = app.oneshot(authed_get(&format!("/{name}"), &token)).await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn mcp_proxy_unknown_service_emits_challenge() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let token = ctx_token();
    let app = mcp::router(&ctx).layer(middleware::from_fn_with_state(token, inject_ctx));
    let resp = app
        .oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri("/ghost-server")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    // ServiceNotFound on the MCP path yields an RFC 9728 challenge or a 4xx.
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn proxy_engine_default_constructs() {
    let engine = systemprompt_api::services::proxy::ProxyEngine::default();
    let cloned = engine.clone();
    drop(cloned);
    drop(engine);
}
