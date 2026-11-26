//! A CLI-originated `plugins mcp call` through the real mounted API router.
//!
//! The full `setup_api_server` router is served on a loopback listener and
//! the CLI's own probe (`systemprompt_mcp_entry::cli::probe`) calls a tool on
//! an internal MCP server whose backend is a stub. The proxy verifies the
//! caller's bearer, so the hop token the backend receives names the user and
//! session of that token, and the context id it forwards is one the caller
//! owns. A context id the caller does not own, and a bearer nobody issued,
//! are refused before the backend sees anything.

use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_test_fixtures::AppContextSeeds as _;

use http::StatusCode;
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::{
    ContextId, McpServerId, McpToolName, ServiceName, SessionToken, UserId,
};
use systemprompt_mcp_entry::cli::call_client::{ToolCallParams, execute_tool_call};
use systemprompt_mcp_entry::cli::probe::McpTarget;
use systemprompt_models::auth::UserType;
use systemprompt_runtime::AppContext;
use systemprompt_security::jwt::{AdminTokenParams, JwtService, decode_mcp_hop};
use systemprompt_test_fixtures::{
    AuthedFixture, seed_user_row, seed_user_row_with_roles, seed_user_session,
};
use uuid::Uuid;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

struct Harness {
    api: String,
    backend: MockServer,
    ctx: Arc<AppContext>,
    server: String,
    caller: AuthedFixture,
    owned: ContextId,
    foreign: ContextId,
    _bootstrap: systemprompt_test_fixtures::TestBootstrap,
    _serve: tokio::task::JoinHandle<()>,
}

fn mcp_range_listener() -> anyhow::Result<std::net::TcpListener> {
    (5000..=5999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .ok_or_else(|| anyhow::anyhow!("no free port in the MCP range 5000-5999"))
}

async fn mount_backend(backend: &MockServer, server: &str) {
    let session = format!("sess-{server}");
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"clientInfo": {"name": format!("systemprompt-cli-{server}")}}
        })))
        .respond_with(move |request: &Request| {
            let body: serde_json::Value = request.body_json().expect("json body");
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", session.as_str())
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "attribution-stub", "version": "1.0.0"}
                    }
                }))
        })
        .mount(backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "echo", "arguments": {"message": "hi"}}
        })))
        .respond_with(|request: &Request| {
            let body: serde_json::Value = request.body_json().expect("json body");
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {"content": [{"type": "text", "text": "attributed"}], "isError": false}
                }))
        })
        .mount(backend)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(backend)
        .await;
}

async fn seed_context(ctx: &AppContext, owner: &UserId) -> anyhow::Result<ContextId> {
    let context = ContextId::generate();
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, 'cli-attribution')",
    )
    .bind(context.as_str())
    .bind(owner.as_str())
    .execute(ctx.db_pool().pool().as_ref())
    .await?;
    Ok(context)
}

async fn harness() -> anyhow::Result<Harness> {
    harness_with(true).await
}

async fn harness_with(oauth_required: bool) -> anyhow::Result<Harness> {
    let backend = MockServer::builder()
        .listener(mcp_range_listener()?)
        .start()
        .await;
    let server = format!("cli_attr_{}", &Uuid::new_v4().simple().to_string()[..10]);
    mount_backend(&backend, &server).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let api = format!("http://{}", listener.local_addr()?);
    let yaml = format!(
        "mcp_servers:\n  {server}:\n    server_type: internal\n    binary: {server}-bin\n    port: {port}\n    enabled: true\n    tool_policy: allow\n    display_in_web: false\n    oauth:\n      required: {oauth_required}\n      scopes: [admin]\n      audience: mcp\n",
        port = backend.address().port()
    );
    let bootstrap = systemprompt_test_fixtures::bootstrap::init_isolated_bootstrap(&api, &yaml);
    let manifest_dir = bootstrap.system_path.join("extensions").join(&server);
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("manifest.yaml"),
        format!("extension:\n  type: mcp\n  name: {server}\n  binary: {server}-bin\n"),
    )?;
    let ctx = systemprompt_test_fixtures::fixture_app_context_on_bootstrap(
        &bootstrap,
        bootstrap.boot_config().as_ref().clone(),
        systemprompt_test_fixtures::fixture_admitted(),
    )
    .await?;
    ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone())
        .create_service(CreateServiceInput {
            name: &ServiceName::new(server.as_str()),
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Running,
            port: backend.address().port(),
            binary_mtime: None,
        })
        .await?;

    let caller = caller_credential(&ctx).await?;
    let other = UserId::new(format!("cli-attr-other-{}", Uuid::new_v4().simple()));
    seed_user_row(ctx.db_pool(), &other, &format!("{other}@example.invalid")).await?;
    let owned = seed_context(&ctx, &caller.user_id).await?;
    let foreign = seed_context(&ctx, &other).await?;

    let router = systemprompt_api::services::server::setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let serve = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    Ok(Harness {
        api,
        backend,
        ctx,
        server,
        caller,
        owned,
        foreign,
        _bootstrap: bootstrap,
        _serve: serve,
    })
}

async fn caller_credential(ctx: &AppContext) -> anyhow::Result<AuthedFixture> {
    let user_id = UserId::generate();
    let session_id = systemprompt_identifiers::SessionId::generate();
    let email = format!("cli-attr-{}@example.invalid", Uuid::new_v4().simple());
    seed_user_row_with_roles(
        ctx.db_pool(),
        &user_id,
        &email,
        &["admin".to_owned(), "user".to_owned()],
    )
    .await?;
    seed_user_session(ctx.db_pool(), &user_id, &session_id).await?;
    let jwt = JwtService::generate_admin_token(
        &systemprompt_test_fixtures::test_signing_keys(),
        &AdminTokenParams {
            user_id: &user_id,
            session_id: &session_id,
            email: &email,
            issuer: &ctx.config().jwt_issuer,
            duration: chrono::Duration::hours(1),
            client_id: None,
        },
    )?;
    Ok(AuthedFixture {
        user_id,
        session_id,
        email,
        jwt,
    })
}

fn session(caller: &AuthedFixture, context: &ContextId) -> CallerSession {
    CallerSession::new(
        SessionToken::new(caller.jwt.as_str()),
        caller.session_id.clone(),
        caller.user_id.clone(),
        Some(context.clone()),
        UserType::Admin,
    )
}

async fn backend_posts(backend: &MockServer) -> Vec<Request> {
    backend
        .received_requests()
        .await
        .expect("recorded backend requests")
        .into_iter()
        .filter(|request| request.method.as_str() == "POST")
        .collect()
}

fn initialize_request(h: &Harness, bearer: &str, context: &ContextId) -> reqwest::RequestBuilder {
    initialize_without_bearer(h, context).header("authorization", format!("Bearer {bearer}"))
}

fn initialize_without_bearer(h: &Harness, context: &ContextId) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(format!("{}/api/v1/mcp/{}/mcp", h.api, h.server))
        .header("accept", "application/json, text/event-stream")
        .header("x-context-id", context.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                       "clientInfo": {"name": format!("systemprompt-cli-{}", h.server), "version": "1"}}
        }))
}

#[tokio::test]
async fn a_cli_call_is_attributed_to_the_verified_token_and_an_owned_context() -> anyhow::Result<()>
{
    let h = harness().await?;
    let caller = session(&h.caller, &h.owned);
    let target = McpTarget::through_proxy(&h.api, McpServerId::new(h.server.as_str()), &caller);
    let result = execute_tool_call(ToolCallParams {
        target: &target,
        tool_name: &McpToolName::new("echo"),
        arguments: Some(serde_json::json!({"message": "hi"})),
        timeout_secs: 20,
    })
    .await?;
    assert_eq!(result.is_error, Some(false));

    let posts = backend_posts(&h.backend).await;
    let call = posts
        .iter()
        .find(|request| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .is_ok_and(|body| body["method"] == "tools/call")
        })
        .expect("the tool call reached the backend");
    let hop = call
        .headers
        .get(systemprompt_identifiers::headers::MCP_HOP)
        .and_then(|value| value.to_str().ok())
        .expect("the proxy attached a hop token");
    let claims = decode_mcp_hop(
        systemprompt_test_fixtures::test_signing_keys().keyring(),
        &h.ctx.config().jwt_issuer,
        &McpServerId::new(h.server.as_str()),
        hop,
    )?;
    assert_eq!(claims.identity.user_id, h.caller.user_id);
    assert_eq!(claims.identity.session_id, h.caller.session_id);
    assert_eq!(
        claims.identity.user.as_ref().map(|user| &user.id),
        Some(&h.caller.user_id),
        "the hop carries the user the proxy verified from the bearer"
    );
    assert_eq!(
        call.headers
            .get(systemprompt_identifiers::headers::CONTEXT_ID)
            .and_then(|value| value.to_str().ok()),
        Some(h.owned.as_str()),
        "the forwarded context is the caller's own"
    );
    Ok(())
}

#[tokio::test]
async fn a_context_the_caller_does_not_own_is_refused_before_the_backend() -> anyhow::Result<()> {
    let h = harness().await?;
    let response = initialize_request(&h, h.caller.jwt.as_str(), &h.foreign)
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(backend_posts(&h.backend).await.is_empty());

    let owned = initialize_request(&h, h.caller.jwt.as_str(), &h.owned)
        .send()
        .await?;
    assert_eq!(
        owned.status(),
        StatusCode::OK,
        "the owned context is admitted"
    );
    Ok(())
}

#[tokio::test]
async fn a_bearer_nobody_issued_is_refused_before_the_backend() -> anyhow::Result<()> {
    let h = harness().await?;
    let response = initialize_request(&h, "not-an-issued-credential", &h.owned)
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(backend_posts(&h.backend).await.is_empty());
    Ok(())
}

fn forwarded_hop(h: &Harness, request: &Request) -> systemprompt_models::auth::McpHopClaims {
    let hop = request
        .headers
        .get(systemprompt_identifiers::headers::MCP_HOP)
        .and_then(|value| value.to_str().ok())
        .expect("the proxy attached a hop token");
    decode_mcp_hop(
        systemprompt_test_fixtures::test_signing_keys().keyring(),
        &h.ctx.config().jwt_issuer,
        &McpServerId::new(h.server.as_str()),
        hop,
    )
    .expect("the hop verifies")
}

#[tokio::test]
async fn identity_headers_that_differ_from_the_token_are_not_the_identity() -> anyhow::Result<()> {
    let h = harness().await?;
    let response = initialize_request(&h, h.caller.jwt.as_str(), &h.owned)
        .header("x-user-id", "forged-user")
        .header("x-session-id", "forged-session")
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let posts = backend_posts(&h.backend).await;
    let claims = forwarded_hop(&h, posts.first().expect("the backend saw the call"));
    assert_eq!(claims.identity.user_id, h.caller.user_id);
    assert_eq!(claims.identity.session_id, h.caller.session_id);
    for request in &posts {
        for (name, value) in &request.headers {
            let value = value.to_str().unwrap_or_default();
            assert!(
                value != "forged-user" && value != "forged-session",
                "header {name} forwards a client-supplied identity"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn on_a_server_without_oauth_the_context_owner_is_still_checked() -> anyhow::Result<()> {
    let h = harness_with(false).await?;
    let foreign = initialize_request(&h, h.caller.jwt.as_str(), &h.foreign)
        .send()
        .await?;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    let unverified = initialize_without_bearer(&h, &h.owned).send().await?;
    assert_eq!(
        unverified.status(),
        StatusCode::FORBIDDEN,
        "a context named without a verifiable caller is refused"
    );
    assert!(backend_posts(&h.backend).await.is_empty());

    let owned = initialize_request(&h, h.caller.jwt.as_str(), &h.owned)
        .send()
        .await?;
    assert_eq!(
        owned.status(),
        StatusCode::OK,
        "the owned context is admitted"
    );
    Ok(())
}
