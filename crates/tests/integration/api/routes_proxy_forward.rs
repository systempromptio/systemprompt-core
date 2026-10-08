//! End-to-end proxy forwarding through a stub backend.
//!
//! Stands up a `wiremock` MockServer as the local backend, declares it as an
//! internal MCP server whose OAuth block requires the `admin` scope, registers
//! a `services` row (status `running`, port = the mock's port), and drives a
//! request through the bare `proxy::agents` / `proxy::mcp` routers so the full
//! forwarding pipeline runs: `ServiceResolver` (DB lookup + running check),
//! `AccessValidator` (OAuth requirement lookup + bearer, audience and scope
//! validation), backend URL building, request-context header injection, the
//! outbound `reqwest` send, and `ResponseHandler` re-assembly of the upstream
//! response.
//!
//! A service the registries do not declare (any `module_name` other than
//! `agent` or `mcp`) requires OAuth but declares no scopes, so the proxy
//! refuses it even with a valid credential.
//!
//! The bare routers carry no middleware, so each request is given a
//! `RequestContext` extension manually (the proxy refuses to forward without
//! one). A self-issued admin JWT — minted with the live `Config` issuer and the
//! process-wide test signing key — satisfies the bearer check.

use std::sync::Once;

use axum::body::Body;
use axum::http::{Request, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use systemprompt_api::routes::proxy::{agents, mcp};
use systemprompt_identifiers::{Actor, AgentName, ContextId, JwtToken, SessionId, TraceId, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_config, init_services_bootstrap,
    install_test_signing_key, mint_admin_jwt, test_app_context, test_db_pool,
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

struct DeclaredBackend {
    name: String,
    ctx: std::sync::Arc<systemprompt_runtime::AppContext>,
    _boot: TestBootstrap,
}

// Why: the services validator only admits MCP ports in 5000-5999, and the
// resolver proxies to the port the registry declares, so the backend must
// listen inside that range.
fn mcp_range_listener() -> anyhow::Result<std::net::TcpListener> {
    (5000..=5999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .ok_or_else(|| anyhow::anyhow!("no free port in the MCP range 5000-5999"))
}

// Why: a port can be free to bind on 127.0.0.1 while a wildcard listener
// (macOS AirPlay holds *:5000) still answers connections to it, so a "dead"
// backend address must be one that refuses a connection once released.
fn dead_mcp_range_address() -> anyhow::Result<std::net::SocketAddr> {
    for port in 5000..=5999u16 {
        let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", port)) else {
            continue;
        };
        let address = listener.local_addr()?;
        drop(listener);
        if std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(200))
            .is_err()
        {
            return Ok(address);
        }
    }
    anyhow::bail!("no dead port in the MCP range 5000-5999")
}

async fn declared_backend(prefix: &str, port: u16) -> anyhow::Result<DeclaredBackend> {
    let name = format!("{prefix}_{}", Uuid::new_v4().simple());
    let services = format!(
        r#"mcp_servers:
  {name}:
    type: internal
    binary: fixture
    package: fixture
    port: {port}
    enabled: true
    tool_policy: allow
    display_in_web: false
    oauth:
      required: true
      scopes: [admin]
      audience: mcp
      client_id: null
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#
    );
    let boot = init_services_bootstrap(&services);
    let manifest_dir = boot.system_path.join("extensions").join("fixture");
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("manifest.yaml"),
        "extension:\n  type: mcp\n  name: fixture\n  binary: fixture\n",
    )?;
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &boot.database_url);
    register_running_service(&pool, &name, "mcp", port).await?;
    Ok(DeclaredBackend {
        name,
        ctx,
        _boot: boot,
    })
}

#[tokio::test]
async fn proxy_reports_a_dead_registered_backend_and_recovers_when_that_backend_returns()
-> anyhow::Result<()> {
    let address = dead_mcp_range_address()?;

    let declared = declared_backend("dead", address.port()).await?;
    let name = &declared.name;
    let token = ctx_token();
    let app = agents::router(&declared.ctx)
        .layer(middleware::from_fn_with_state(token.clone(), inject_ctx));

    let unavailable = app
        .clone()
        .oneshot(authed_get(&format!("/{name}/health"), &token))
        .await?;
    let (status, body) = body_to_string(unavailable).await?;
    assert_eq!(status, http::StatusCode::BAD_GATEWAY, "{body}");
    assert!(
        body.is_empty(),
        "the proxy does not expose transport internals: {body}"
    );
    assert!(
        !body.contains(&token),
        "a connection diagnostic must not echo the caller credential: {body}"
    );

    let listener = std::net::TcpListener::bind(address)?;
    let backend = MockServer::builder().listener(listener).start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_string("recovered"))
        .expect(1)
        .mount(&backend)
        .await;
    let recovered = app
        .oneshot(authed_get(&format!("/{name}/health"), &token))
        .await?;
    let (status, body) = body_to_string(recovered).await?;
    assert_eq!(status, http::StatusCode::OK, "{body}");
    assert_eq!(body, "recovered");
    Ok(())
}

fn ctx_token() -> String {
    if !Config::is_initialized() {
        ensure_config();
    }
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
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
    .with_auth_token(JwtToken::new(token));
    req.extensions_mut().insert(rc);
    next.run(req).await
}

async fn register_running_service(
    pool: &systemprompt_database::DbPool,
    name: &str,
    module: &str,
    port: u16,
) -> anyhow::Result<()> {
    systemprompt_test_fixtures::seed_running_service(pool, name, module, port).await
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

async fn declared_mock() -> anyhow::Result<(MockServer, DeclaredBackend)> {
    let backend = MockServer::builder()
        .listener(mcp_range_listener()?)
        .start()
        .await;
    let declared = declared_backend("fwd", backend.address().port()).await?;
    Ok((backend, declared))
}

#[tokio::test]
async fn agent_proxy_forwards_get_to_backend() -> anyhow::Result<()> {
    let (backend, declared) = declared_mock().await?;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("backend-ok"))
        .mount(&backend)
        .await;

    let token = ctx_token();
    let app = agents::router(&declared.ctx)
        .layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_get(&format!("/{}", declared.name), &token))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("backend-ok"), "{body}");
    Ok(())
}

#[tokio::test]
async fn agent_proxy_forwards_subpath_with_query() -> anyhow::Result<()> {
    let (backend, declared) = declared_mock().await?;
    Mock::given(method("GET"))
        .and(path("/api/v1/items"))
        .respond_with(ResponseTemplate::new(200).set_body_string("items-list"))
        .mount(&backend)
        .await;

    let token = ctx_token();
    let app = agents::router(&declared.ctx)
        .layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_get(
            &format!("/{}/api/v1/items?limit=5", declared.name),
            &token,
        ))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("items-list"), "{body}");
    Ok(())
}

#[tokio::test]
async fn agent_proxy_forwards_post_body() -> anyhow::Result<()> {
    let (backend, declared) = declared_mock().await?;
    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(ResponseTemplate::new(201).set_body_string("created"))
        .mount(&backend)
        .await;

    let token = ctx_token();
    let app = agents::router(&declared.ctx)
        .layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_post(
            &format!("/{}/submit", declared.name),
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
    let (backend, declared) = declared_mock().await?;
    Mock::given(method("GET"))
        .and(path("/boom"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .mount(&backend)
        .await;

    let token = ctx_token();
    let app = agents::router(&declared.ctx)
        .layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_get(&format!("/{}/boom", declared.name), &token))
        .await?;
    assert_eq!(resp.status().as_u16(), 503);
    Ok(())
}

#[tokio::test]
async fn an_undeclared_module_is_forbidden_even_with_a_valid_credential() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("must-not-be-reached"))
        .expect(0)
        .mount(&backend)
        .await;

    let name = unique_name("custom-fwd");
    register_running_service(&pool, &name, "custom", backend.address().port()).await?;

    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app.oneshot(authed_get(&format!("/{name}"), &token)).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(
        status.is_client_error() || status.is_server_error(),
        "a services row with an undeclared module is refused, got {status}: {body}"
    );
    assert!(!body.contains("must-not-be-reached"), "{body}");
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

// `lookup_oauth_requirement` branches on the services row's `module_name`. The
// `agent` and `mcp` branches consult their registries, and a row naming a
// service the registry does not know is exactly the stale-row case that must
// not be forwarded.
#[tokio::test]
async fn a_services_row_naming_an_unknown_agent_is_not_proxied() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("must-not-be-reached"))
        .mount(&backend)
        .await;

    let name = unique_name("ghost-agent");
    register_running_service(&pool, &name, "agent", backend.address().port()).await?;

    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app.oneshot(authed_get(&format!("/{name}"), &token)).await?;
    let (status, body) = body_to_string(resp).await?;

    assert!(
        status.is_client_error() || status.is_server_error(),
        "a running row for an agent the registry never declared must not forward, got {status}"
    );
    assert!(
        !body.contains("must-not-be-reached"),
        "the backend must never have been dialled: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_services_row_naming_an_unknown_mcp_server_is_not_proxied() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_string("must-not-be-reached"))
        .mount(&backend)
        .await;

    let name = unique_name("ghost-mcp");
    register_running_service(&pool, &name, "mcp", backend.address().port()).await?;

    let token = ctx_token();
    let app = mcp::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(authed_post(&format!("/{name}/mcp"), &token, "{}"))
        .await?;
    let (status, body) = body_to_string(resp).await?;

    assert!(
        status.is_client_error() || status.is_server_error(),
        "a running row for an MCP server the registry never declared must not forward, got {status}"
    );
    assert!(
        !body.contains("must-not-be-reached"),
        "the backend must never have been dialled: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unknown_module_name_still_demands_a_credential() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("must-not-be-reached"))
        .mount(&backend)
        .await;

    let name = unique_name("unknown-module");
    register_running_service(&pool, &name, "something-else", backend.address().port()).await?;

    // No Authorization header: a row whose module is outside the closed
    // vocabulary is a corrupt registry row and must fail closed.
    let token = ctx_token();
    let app = agents::router(&ctx).layer(middleware::from_fn_with_state(token.clone(), inject_ctx));
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/{name}"))
                .body(Body::empty())
                .expect("request build"),
        )
        .await?;
    let (status, body) = body_to_string(resp).await?;

    assert!(
        status.is_client_error() || status.is_server_error(),
        "an unknown module must fail closed, got {status}: {body}"
    );
    assert!(!body.contains("must-not-be-reached"), "{body}");
    Ok(())
}
