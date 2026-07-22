//! External-MCP proxying through `ProxyEngine::proxy_request` — the registry
//! diverts external servers to `proxy_external_mcp`, which mints a per-user
//! provider bearer via the `external_auth` accessor, forwards the MCP frame to
//! the provider, audits client-mediated `tools/call`s, and maps resolver
//! failures. The internal-registry branch forwards to the local backend port
//! with injected context headers.
//!
//! Each test boots an isolated profile whose `api_*_url`s point at a wiremock
//! server (so the bearer accessor resolves there) and whose services config
//! seeds the `mcp_servers:` registry — one process per test under nextest.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::Request;
use http::StatusCode;
use systemprompt_database::{CreateServiceInput, DbPool, ServiceRepository};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PROVIDER_BEARER: &str = "prov-tok-abc";
const CALLER_JWT: &str = "caller-systemprompt-jwt";

struct Harness {
    app: axum::Router,
    pool: DbPool,
    server: MockServer,
    ctx: Arc<AppContext>,
    ext_name: String,
    int_name: String,
    _bootstrap: systemprompt_test_fixtures::TestBootstrap,
}

fn services_yaml(provider_url: &str, ext_name: &str, int_name: &str) -> String {
    format!(
        r#"mcp_servers:
  {ext_name}:
    type: external
    binary: {ext_name}
    port: 5990
    endpoint: {provider_url}
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
    external_auth:
      token_endpoint: /ext-token
    headers:
      x-provider-static: static-val
  {int_name}:
    type: internal
    binary: {int_name}-bin
    port: 5321
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
"#
    )
}

async fn harness() -> anyhow::Result<Harness> {
    let server = MockServer::start().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let ext_name = format!("cov-ext-{}", &suffix[..8]);
    let int_name = format!("cov-int-{}", &suffix[..8]);
    let provider_url = format!("{}/provider/mcp", server.uri());
    let yaml = services_yaml(&provider_url, &ext_name, &int_name);
    let b = systemprompt_test_fixtures::bootstrap::init_isolated_bootstrap(&server.uri(), &yaml);

    let manifest_dir = b.system_path.join("extensions").join(&int_name);
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("manifest.yaml"),
        format!("extension:\n  type: mcp\n  name: {int_name}\n  binary: {int_name}-bin\n"),
    )?;

    let pool = systemprompt_test_fixtures::fixture_db_pool(&b.database_url).await?;
    let paths = PathsConfig {
        system: b.system_path.to_string_lossy().into_owned(),
        services: b.services_path.to_string_lossy().into_owned(),
        bin: b.bin_path.to_string_lossy().into_owned(),
        web_path: None,
        storage: Some(b.storage_path.to_string_lossy().into_owned()),
        geoip_database: None,
    };
    let ctx = systemprompt_test_fixtures::fixture_app_context_with(
        &pool,
        &b.database_url,
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )?;
    let app = systemprompt_api::routes::proxy::mcp::router(&ctx);
    Ok(Harness {
        app,
        pool,
        server,
        ctx,
        ext_name,
        int_name,
        _bootstrap: b,
    })
}

fn caller_context(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("proxy-test-agent"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(user)))
    .with_auth_token(CALLER_JWT)
}

fn tool_call_body(tool: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": tool, "arguments": {"q": "x"}}
    })
    .to_string()
}

fn proxied_post(service: &str, body: String, ctx: Option<RequestContext>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(http::Method::POST)
        .uri(format!("/{service}/mcp"))
        .header("content-type", "application/json")
        .header("mcp-session-id", "sess-ext-1")
        .header("x-secret", "must-not-forward");
    if let Some(rc) = ctx {
        builder = builder.extension(rc);
    }
    builder.body(Body::from(body)).expect("request build")
}

async fn mount_accessor(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/ext-token"))
        .and(header("authorization", format!("Bearer {CALLER_JWT}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": PROVIDER_BEARER
        })))
        .mount(server)
        .await;
}

async fn wait_for_execution_row(pool: &DbPool, tool: &str) -> Option<(String, String)> {
    let p = pool.pool_arc().expect("read pool");
    for _ in 0..100 {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT status, server_name FROM mcp_tool_executions WHERE tool_name = $1",
        )
        .bind(tool)
        .fetch_optional(p.as_ref())
        .await
        .expect("query executions");
        if row.is_some() {
            return row;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

#[tokio::test]
async fn external_tools_call_mints_bearer_forwards_and_audits() -> anyhow::Result<()> {
    let h = harness().await?;
    mount_accessor(&h.server).await;
    let upstream_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "result": {"content": [{"type": "text", "text": "ext-ok"}]}
    })
    .to_string();
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .and(header("authorization", format!("Bearer {PROVIDER_BEARER}")))
        .and(header("x-provider-static", "static-val"))
        .and(header("mcp-session-id", "sess-ext-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(upstream_body.clone(), "application/json"),
        )
        .mount(&h.server)
        .await;

    let tool = format!("ext-tool-{}", Uuid::new_v4().simple());
    let resp = h
        .app
        .oneshot(proxied_post(
            &h.ext_name,
            tool_call_body(&tool),
            Some(caller_context("ext-user")),
        ))
        .await?;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(String::from_utf8_lossy(&bytes), upstream_body);

    let provider_reqs: Vec<_> = h
        .server
        .received_requests()
        .await
        .expect("recorded requests")
        .into_iter()
        .filter(|r| r.url.path() == "/provider/mcp")
        .collect();
    assert_eq!(provider_reqs.len(), 1);
    assert!(
        provider_reqs[0].headers.get("x-secret").is_none(),
        "client headers outside the passthrough set must be withheld"
    );
    assert_eq!(
        provider_reqs[0]
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok()),
        Some(format!("Bearer {PROVIDER_BEARER}").as_str()),
        "the systemprompt JWT must be replaced by the provider bearer"
    );

    let (exec_status, server_name) = wait_for_execution_row(&h.pool, &tool)
        .await
        .expect("tools/call audited under the external server");
    assert_eq!(server_name, h.ext_name);
    assert!(!exec_status.is_empty());
    Ok(())
}

#[tokio::test]
async fn external_non_tool_call_passes_through_without_audit() -> anyhow::Result<()> {
    let h = harness().await?;
    mount_accessor(&h.server).await;
    let upstream_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {"protocolVersion": "2025-06-18"}
    })
    .to_string();
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(upstream_body.clone(), "application/json"),
        )
        .mount(&h.server)
        .await;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    })
    .to_string();
    let resp = h
        .app
        .oneshot(proxied_post(
            &h.ext_name,
            body,
            Some(caller_context("ext-user-2")),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    assert_eq!(String::from_utf8_lossy(&bytes), upstream_body);

    let p = h.pool.pool_arc().expect("read pool");
    let audited: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_tool_executions WHERE server_name = $1")
            .bind(&h.ext_name)
            .fetch_one(p.as_ref())
            .await?;
    assert_eq!(
        audited, 0,
        "initialize is not a tools/call and is not audited"
    );
    Ok(())
}

#[tokio::test]
async fn external_accessor_without_banked_token_is_service_unavailable() -> anyhow::Result<()> {
    let h = harness().await?;
    Mock::given(method("GET"))
        .and(path("/ext-token"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&h.server)
        .await;

    let resp = h
        .app
        .oneshot(proxied_post(
            &h.ext_name,
            tool_call_body("nope"),
            Some(caller_context("ext-user-3")),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    assert!(body.contains("connect the provider account"), "{body}");
    Ok(())
}

#[tokio::test]
async fn external_without_request_context_is_unauthorized() -> anyhow::Result<()> {
    let h = harness().await?;
    let resp = h
        .app
        .oneshot(proxied_post(&h.ext_name, tool_call_body("nope"), None))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn external_with_anonymous_context_is_unauthorized() -> anyhow::Result<()> {
    let h = harness().await?;
    let anon = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("proxy-test-agent"),
    );
    let resp = h
        .app
        .oneshot(proxied_post(
            &h.ext_name,
            tool_call_body("nope"),
            Some(anon),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn internal_registry_server_forwards_to_backend_with_context_headers() -> anyhow::Result<()> {
    let h = harness().await?;
    let backend_port = h.server.address().port();
    let repo = ServiceRepository::new(h.ctx.db_pool())?;
    repo.create_service(CreateServiceInput {
        name: &h.int_name,
        module_name: "mcp",
        status: "running",
        port: backend_port,
        binary_mtime: None,
    })
    .await?;

    let upstream_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "result": {"tools": []}
    })
    .to_string();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-int-9")
                .set_body_raw(upstream_body.clone(), "application/json"),
        )
        .mount(&h.server)
        .await;

    let resp = h
        .app
        .oneshot(proxied_post(
            &h.int_name,
            tool_call_body("int-tool"),
            Some(caller_context("int-user")),
        ))
        .await?;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(String::from_utf8_lossy(&bytes), upstream_body);

    let backend_reqs: Vec<_> = h
        .server
        .received_requests()
        .await
        .expect("recorded requests")
        .into_iter()
        .filter(|r| r.url.path() == "/mcp")
        .collect();
    assert_eq!(backend_reqs.len(), 1);
    assert!(
        backend_reqs[0].headers.get("x-trace-id").is_some(),
        "forwarded request carries injected context headers"
    );
    assert_eq!(
        backend_reqs[0]
            .headers
            .get("x-agent-name")
            .and_then(|v| v.to_str().ok()),
        Some(h.int_name.as_str()),
        "agent name is rewritten to the resolved service"
    );
    Ok(())
}
