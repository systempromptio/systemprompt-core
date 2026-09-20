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

use super::common::assert_forwarded_with_execution_stamp;
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
const BROKER_SECRET: &str = "cov-broker-secret";

struct Harness {
    app: axum::Router,
    pool: DbPool,
    server: MockServer,
    backend: MockServer,
    ctx: Arc<AppContext>,
    ext_name: String,
    int_name: String,
    _bootstrap: systemprompt_test_fixtures::TestBootstrap,
    _database: Option<systemprompt_test_fixtures::DisposableDb>,
}

fn services_yaml(provider_url: &str, ext_name: &str, int_name: &str, int_port: u16) -> String {
    format!(
        r#"mcp_servers:
  {ext_name}:
    type: external
    endpoint: {provider_url}
    enabled: true
    tool_policy: allow
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
    port: {int_port}
    enabled: true
    tool_policy: allow
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
"#
    )
}

const ENFORCED_SECRET_SCAN_GOVERNANCE: &str = "governance:
  mode: enforce
  policies:
    - id: secret_scan
      patterns:
        - id: cloud-access-key
          name: Cloud Access Key
          regex: 'AKIA[0-9A-Z]{16}'
          redact_whole_value: true
";

// Why: the services validator only admits MCP ports in 5000-5999, and the
// resolver proxies to the port the registry declares, so the backend mock
// must listen inside that range.
fn mcp_range_listener() -> anyhow::Result<std::net::TcpListener> {
    for port in 5000..6000u16 {
        if let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", port)) {
            return Ok(listener);
        }
    }
    anyhow::bail!("no free port in the MCP range 5000-5999")
}

async fn harness() -> anyhow::Result<Harness> {
    harness_with_governance(None).await
}

async fn harness_with_governance(governance_yaml: Option<&str>) -> anyhow::Result<Harness> {
    harness_with_database(governance_yaml, None, None).await
}

async fn private_harness(label: &str) -> anyhow::Result<Harness> {
    let database = systemprompt_test_fixtures::DisposableDb::installed(label).await?;
    harness_with_database(None, Some(database), None).await
}

async fn harness_with_database(
    governance_yaml: Option<&str>,
    database: Option<systemprompt_test_fixtures::DisposableDb>,
    provider_url_override: Option<&str>,
) -> anyhow::Result<Harness> {
    systemprompt_test_fixtures::install_named_secret(
        systemprompt_mcp::services::client::external_auth::BROKER_SECRET_KEY,
        BROKER_SECRET,
    );
    let server = MockServer::start().await;
    let backend = MockServer::builder()
        .listener(mcp_range_listener()?)
        .start()
        .await;
    let suffix = Uuid::new_v4().simple().to_string();
    let ext_name = format!("cov-ext-{}", &suffix[..8]);
    let int_name = format!("cov-int-{}", &suffix[..8]);
    let provider_url = provider_url_override
        .map_or_else(|| format!("{}/provider/mcp", server.uri()), str::to_owned);
    let yaml = services_yaml(
        &provider_url,
        &ext_name,
        &int_name,
        backend.address().port(),
    );
    let b = systemprompt_test_fixtures::bootstrap::init_isolated_bootstrap(&server.uri(), &yaml);
    if let Some(governance) = governance_yaml {
        let governance_dir = b.services_path.join("governance");
        std::fs::create_dir_all(&governance_dir)?;
        std::fs::write(governance_dir.join("config.yaml"), governance)?;
    }

    let manifest_dir = b.system_path.join("extensions").join(&int_name);
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("manifest.yaml"),
        format!("extension:\n  type: mcp\n  name: {int_name}\n  binary: {int_name}-bin\n"),
    )?;

    let database_url = database.as_ref().map_or(
        b.database_url.as_str(),
        systemprompt_test_fixtures::DisposableDb::url,
    );
    let pool = if let Some(database) = &database {
        database.pool().await?
    } else {
        systemprompt_test_fixtures::fixture_db_pool(database_url).await?
    };
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
        database_url,
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )?;
    let app = systemprompt_api::routes::proxy::mcp::router(&ctx);
    Ok(Harness {
        app,
        pool,
        server,
        backend,
        ctx,
        ext_name,
        int_name,
        _bootstrap: b,
        _database: database,
    })
}

fn caller_context(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("proxy-test-agent").expect("valid AgentName"),
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
        .and(header("x-systemprompt-credential-broker", BROKER_SECRET))
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
    assert_forwarded_with_execution_stamp(&bytes, &upstream_body);

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
        AgentName::try_new("proxy-test-agent").expect("valid AgentName"),
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
    let backend_port = h.backend.address().port();
    let repo = ServiceRepository::new(
        h.ctx.db_pool(),
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )?;
    // Why: the row carries a port from an earlier run under another offset;
    // the resolver must trust the port this instance spawns, not the row.
    let stale_port = 5321;
    repo.create_service(CreateServiceInput {
        name: &h.int_name,
        module_name: "mcp",
        status: "running",
        port: stale_port,
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
        .mount(&h.backend)
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
        .backend
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
        Some("proxy-test-agent"),
        "the caller's agent name reaches the backend; the reverse proxy does not \
         substitute the callee server's name for it"
    );
    let row = repo
        .find_service_by_name(&h.int_name)
        .await?
        .expect("service row");
    assert_eq!(
        row.port,
        i32::from(backend_port),
        "the stale row is rewritten to the port this instance spawns"
    );
    Ok(())
}

#[tokio::test]
async fn external_secret_is_denied_before_provider_receives_call() -> anyhow::Result<()> {
    let h = harness_with_governance(Some(ENFORCED_SECRET_SCAN_GOVERNANCE)).await?;
    mount_accessor(&h.server).await;
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&h.server)
        .await;
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "upload", "arguments": {"key": "AKIAIOSFODNN7EXAMPLE"}}
    })
    .to_string();
    let response = h
        .app
        .oneshot(proxied_post(
            &h.ext_name,
            body,
            Some(caller_context("secret-user")),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn external_unknown_session_is_rejected_before_forwarding() -> anyhow::Result<()> {
    let h = harness().await?;
    mount_accessor(&h.server).await;
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&h.server)
        .await;
    let mut request = proxied_post(
        &h.ext_name,
        tool_call_body("read"),
        Some(caller_context("session-user")),
    );
    request
        .headers_mut()
        .insert("mcp-session-id", "never-initialized".parse()?);
    let response = h.app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn external_malformed_tool_call_is_not_forwarded() -> anyhow::Result<()> {
    let h = harness().await?;
    mount_accessor(&h.server).await;
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&h.server)
        .await;
    for body in ["{", "[]", r#"{"method":"tools/call","params":{}}"#] {
        let response = h
            .app
            .clone()
            .oneshot(proxied_post(
                &h.ext_name,
                body.to_owned(),
                Some(caller_context("malformed-user")),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    Ok(())
}

#[tokio::test]
async fn external_initialized_session_survives_failed_delete_and_rejects_other_user()
-> anyhow::Result<()> {
    let h = harness().await?;
    mount_accessor(&h.server).await;
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "initialized-session")
                .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})),
        )
        .mount(&h.server)
        .await;
    let body =
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}).to_string();
    let response = h
        .app
        .clone()
        .oneshot(proxied_post(
            &h.ext_name,
            body,
            Some(caller_context("session-owner")),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["mcp-session-id"], "initialized-session");
    let followup = |user: &str, method: http::Method| {
        let mut request = proxied_post(&h.ext_name, String::new(), Some(caller_context(user)));
        *request.method_mut() = method;
        request
            .headers_mut()
            .insert("mcp-session-id", "initialized-session".parse().unwrap());
        request
    };
    let response = h
        .app
        .clone()
        .oneshot(followup("other-user", http::Method::GET))
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Mock::given(method("GET"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&h.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&h.server)
        .await;
    let response = h
        .app
        .clone()
        .oneshot(followup("session-owner", http::Method::DELETE))
        .await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let response = h
        .app
        .clone()
        .oneshot(followup("session-owner", http::Method::GET))
        .await?;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "failed DELETE preserves ownership"
    );
    h.server.reset().await;
    mount_accessor(&h.server).await;
    Mock::given(method("DELETE"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;
    let response = h
        .app
        .clone()
        .oneshot(followup("session-owner", http::Method::DELETE))
        .await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = h
        .app
        .clone()
        .oneshot(followup("session-owner", http::Method::GET))
        .await?;
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "successful DELETE invalidates the binding"
    );
    Ok(())
}

#[tokio::test]
async fn external_session_insert_failure_is_fail_closed_and_retry_persists_one_binding()
-> anyhow::Result<()> {
    let mut h = private_harness("external_session_insert_recovery").await?;
    mount_accessor(&h.server).await;
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "durable-session")
                .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})),
        )
        .mount(&h.server)
        .await;
    let write = h.pool.write_pool_arc()?;
    sqlx::raw_sql(
        "CREATE FUNCTION reject_external_session_insert() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'owned external session fault'; END $$; \
         CREATE TRIGGER reject_external_session_insert BEFORE INSERT ON mcp_external_sessions \
         FOR EACH ROW EXECUTE FUNCTION reject_external_session_insert()",
    )
    .execute(write.as_ref())
    .await?;
    let initialize = || {
        proxied_post(
            &h.ext_name,
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
                .to_string(),
            Some(caller_context("durable-session-owner")),
        )
    };

    let failed = h.app.clone().oneshot(initialize()).await?;
    assert_eq!(failed.status(), StatusCode::FORBIDDEN);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mcp_external_sessions WHERE server_name = $1 AND session_id = $2",
    )
    .bind(&h.ext_name)
    .bind("durable-session")
    .fetch_one(write.as_ref())
    .await?;
    assert_eq!(count, 0, "a failed binding write leaves no session");

    sqlx::raw_sql(
        "DROP TRIGGER reject_external_session_insert ON mcp_external_sessions; \
         DROP FUNCTION reject_external_session_insert()",
    )
    .execute(write.as_ref())
    .await?;
    let retry = h.app.clone().oneshot(initialize()).await?;
    assert_eq!(retry.status(), StatusCode::OK);
    assert_eq!(retry.headers()["mcp-session-id"], "durable-session");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mcp_external_sessions WHERE server_name = $1 AND session_id = $2",
    )
    .bind(&h.ext_name)
    .bind("durable-session")
    .fetch_one(write.as_ref())
    .await?;
    assert_eq!(count, 1, "retry creates exactly one durable binding");

    Mock::given(method("GET"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&h.server)
        .await;
    let mut followup = proxied_post(
        &h.ext_name,
        String::new(),
        Some(caller_context("durable-session-owner")),
    );
    *followup.method_mut() = http::Method::GET;
    followup
        .headers_mut()
        .insert("mcp-session-id", "durable-session".parse()?);
    assert_eq!(
        h.app.clone().oneshot(followup).await?.status(),
        StatusCode::OK
    );
    let provider_posts = h
        .server
        .received_requests()
        .await
        .expect("recorded provider requests")
        .into_iter()
        .filter(|request| {
            request.method == http::Method::POST && request.url.path() == "/provider/mcp"
        })
        .count();
    assert_eq!(
        provider_posts, 2,
        "failed persistence and retry each dispatch once"
    );
    drop(write);
    h._database
        .take()
        .expect("private external-session database")
        .drop_now()
        .await;
    Ok(())
}
// Apply after the private-database harness refactor in proxy_external_mcp.rs.
fn caller_context_with_token(user: &str, token: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("proxy-test-agent").expect("valid AgentName"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(user)))
    .with_auth_token(token)
}

async fn mount_accessor_token(server: &MockServer, caller: &str, provider: &str) {
    Mock::given(method("GET"))
        .and(path("/ext-token"))
        .and(header("authorization", format!("Bearer {caller}")))
        .and(header("x-systemprompt-credential-broker", BROKER_SECRET))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": provider
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn external_session_is_bound_to_provider_credential_and_rotation_does_not_steal_it()
-> anyhow::Result<()> {
    let mut h = private_harness("external_session_credential_rotation").await?;
    const CALLER_A: &str = "caller-jwt-a";
    const CALLER_B: &str = "caller-jwt-b";
    mount_accessor_token(&h.server, CALLER_A, "provider-bearer-a").await;
    mount_accessor_token(&h.server, CALLER_B, "provider-bearer-b").await;
    Mock::given(method("POST"))
        .and(path("/provider/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "credential-bound-session")
                .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})),
        )
        .expect(1)
        .mount(&h.server)
        .await;
    let initialize = proxied_post(
        &h.ext_name,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}).to_string(),
        Some(caller_context_with_token("stable-owner", CALLER_A)),
    );
    let initialized = h.app.clone().oneshot(initialize).await?;
    assert_eq!(initialized.status(), StatusCode::OK);
    let binding_before: (String, Vec<u8>) = sqlx::query_as(
        "SELECT user_id, credential_hash FROM mcp_external_sessions \
         WHERE server_name = $1 AND session_id = $2",
    )
    .bind(&h.ext_name)
    .bind("credential-bound-session")
    .fetch_one(h.pool.pool_arc()?.as_ref())
    .await?;
    assert_eq!(binding_before.0, "stable-owner");
    assert_eq!(binding_before.1.len(), 32);

    Mock::given(method("GET"))
        .and(path("/provider/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&h.server)
        .await;
    let followup = |caller: &str| {
        let mut request = proxied_post(
            &h.ext_name,
            String::new(),
            Some(caller_context_with_token("stable-owner", caller)),
        );
        *request.method_mut() = http::Method::GET;
        request.headers_mut().insert(
            "mcp-session-id",
            "credential-bound-session".parse().expect("session header"),
        );
        request
    };
    let rotated = h.app.clone().oneshot(followup(CALLER_B)).await?;
    assert_eq!(rotated.status(), StatusCode::NOT_FOUND);
    let original = h.app.clone().oneshot(followup(CALLER_A)).await?;
    assert_eq!(original.status(), StatusCode::OK);

    let provider_requests = h
        .server
        .received_requests()
        .await
        .expect("recorded requests")
        .into_iter()
        .filter(|request| request.url.path() == "/provider/mcp")
        .collect::<Vec<_>>();
    assert_eq!(
        provider_requests.len(),
        2,
        "rotated credential is rejected before forwarding"
    );
    assert_eq!(
        provider_requests[0].headers["authorization"],
        "Bearer provider-bearer-a"
    );
    assert_eq!(
        provider_requests[1].headers["authorization"],
        "Bearer provider-bearer-a"
    );

    let binding_after: (String, Vec<u8>) = sqlx::query_as(
        "SELECT user_id, credential_hash FROM mcp_external_sessions \
         WHERE server_name = $1 AND session_id = $2",
    )
    .bind(&h.ext_name)
    .bind("credential-bound-session")
    .fetch_one(h.pool.pool_arc()?.as_ref())
    .await?;
    assert_eq!(binding_after.0, binding_before.0);
    assert_eq!(
        binding_after.1, binding_before.1,
        "rejected rotation leaves the durable credential identity unchanged"
    );
    h._database
        .take()
        .expect("private external-session database")
        .drop_now()
        .await;
    Ok(())
}
async fn private_harness_with_provider(label: &str, provider_url: &str) -> anyhow::Result<Harness> {
    let database = systemprompt_test_fixtures::DisposableDb::installed(label).await?;
    harness_with_database(None, Some(database), Some(provider_url)).await
}


struct RawProviderTask(Option<tokio::task::JoinHandle<anyhow::Result<usize>>>);

impl Drop for RawProviderTask {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}

impl RawProviderTask {
    async fn join(mut self) -> anyhow::Result<usize> {
        let joined = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.0.as_mut().expect("raw provider task"),
        )
        .await
        .map_err(|_| anyhow::anyhow!("raw provider join timed out"))?;
        self.0.take();
        Ok(joined??)
    }
}

async fn raw_session_provider() -> anyhow::Result<(String, RawProviderTask)> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/mcp", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        for attempt in 0..2 {
            let (mut stream, _) =
                tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
                    .await
                    .map_err(|_| anyhow::anyhow!("provider accept timed out"))??;
            let mut request = Vec::new();
            let (header_end, content_length) = loop {
                let mut chunk = [0_u8; 1024];
                let read = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    stream.read(&mut chunk),
                )
                .await
                .map_err(|_| anyhow::anyhow!("provider request read timed out"))??;
                anyhow::ensure!(read != 0, "provider request ended before headers");
                request.extend_from_slice(&chunk[..read]);
                if let Some(offset) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let header_end = offset + 4;
                    let headers = std::str::from_utf8(&request[..header_end])?;
                    let content_length = headers
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        .map(|(_, value)| value.trim().parse::<usize>())
                        .transpose()?
                        .unwrap_or(0);
                    break (header_end, content_length);
                }
                anyhow::ensure!(
                    request.len() <= 64 * 1024,
                    "provider request headers too large"
                );
            };
            while request.len() < header_end + content_length {
                let mut chunk = [0_u8; 1024];
                let read = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    stream.read(&mut chunk),
                )
                .await
                .map_err(|_| anyhow::anyhow!("provider body read timed out"))??;
                anyhow::ensure!(read != 0, "provider request ended before body");
                request.extend_from_slice(&chunk[..read]);
            }
            anyhow::ensure!(request.starts_with(b"POST /mcp HTTP/1.1\r\n"));
            let mut response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\nmcp-session-id: ",
                body.len()
            )
            .into_bytes();
            if attempt == 0 {
                response.push(0xff);
            } else {
                response.extend_from_slice(b"repaired-session");
            }
            response.extend_from_slice(b"\r\n\r\n");
            response.extend_from_slice(body);
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                stream.write_all(&response),
            )
            .await
            .map_err(|_| anyhow::anyhow!("provider response write timed out"))??;
            stream.shutdown().await?;
        }
        Ok(2)
    });
    Ok((url, RawProviderTask(Some(task))))
}

#[tokio::test]
async fn invalid_external_session_header_is_fail_closed_and_valid_retry_binds() -> anyhow::Result<()>
{
    let (provider_url, provider) = raw_session_provider().await?;
    let mut h =
        private_harness_with_provider("external_invalid_session_header", &provider_url).await?;
    mount_accessor(&h.server).await;
    let initialize = || {
        proxied_post(
            &h.ext_name,
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
                .to_string(),
            Some(caller_context("invalid-header-owner")),
        )
    };

    let invalid = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        h.app.clone().oneshot(initialize()),
    )
    .await
    .map_err(|_| anyhow::anyhow!("invalid-header request timed out"))??;
    assert_eq!(invalid.status(), StatusCode::FORBIDDEN);
    let write = h.pool.write_pool_arc()?;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_external_sessions WHERE server_name = $1")
            .bind(&h.ext_name)
            .fetch_one(write.as_ref())
            .await?;
    assert_eq!(count, 0);

    let repaired = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        h.app.clone().oneshot(initialize()),
    )
    .await
    .map_err(|_| anyhow::anyhow!("valid retry timed out"))??;
    assert_eq!(repaired.status(), StatusCode::OK);
    assert_eq!(repaired.headers()["mcp-session-id"], "repaired-session");
    let binding: (String, String, i64) = sqlx::query_as(
        "SELECT user_id, session_id, COUNT(*) OVER() FROM mcp_external_sessions \
         WHERE server_name = $1",
    )
    .bind(&h.ext_name)
    .fetch_one(write.as_ref())
    .await?;
    assert_eq!(
        binding,
        (
            "invalid-header-owner".to_owned(),
            "repaired-session".to_owned(),
            1
        )
    );
    assert_eq!(provider.join().await?, 2);
    drop(write);
    h._database
        .take()
        .expect("private database")
        .drop_now()
        .await;
    Ok(())
}
