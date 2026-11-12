//! The agent route's single body limit, end to end.
//!
//! The A2A context middleware reads the JSON-RPC envelope to find the context
//! id, so it buffers the request; it must accept exactly what the agent proxy
//! behind it accepts. The route is composed here the way the server mounts it:
//! `proxy::agents::router` under `DefaultBodyLimit` and `with_auth` with an
//! `A2AContextMiddleware`, both given `AGENT_REQUEST_BODY_LIMIT_BYTES`.

use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode, header};
use bytes::Bytes;
use systemprompt_api::routes::proxy::agents;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, AuthzPolicy, BodyBudget, JtiRevocationChecker, JwtContextExtractor,
    RouterExt,
};
use systemprompt_models::net::{AGENT_BUFFERED_BYTES_BUDGET, AGENT_REQUEST_BODY_LIMIT_BYTES};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, init_services_bootstrap, seed_admin_credential, seed_running_service,
    test_app_context_on, test_db_pool,
};
use systemprompt_traits::UserProvider;
use systemprompt_users::UserService;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request as MockRequest, ResponseTemplate};

const FILE_PART_BYTES: usize = 20 * 1024 * 1024;

struct Agent {
    name: String,
    ctx: Arc<AppContext>,
    _boot: TestBootstrap,
}

fn agent_range_listener() -> anyhow::Result<std::net::TcpListener> {
    (4000..=4999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .ok_or_else(|| anyhow::anyhow!("no free port in the agent range 4000-4999"))
}

async fn declared_agent(port: u16) -> anyhow::Result<Agent> {
    let name = format!("bodylim_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let services = format!(
        r#"agents:
  {name}:
    name: {name}
    port: {port}
    endpoint: /api/v1/agents/{name}/
    enabled: true
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {name}
      displayName: Body Limit Agent
      description: Body limit fixture
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities: {{ streaming: false, pushNotifications: false }}
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
    metadata:
      systemPrompt: Fixture.
      mcpServers: {{ source: explicit }}
      skills: {{ source: explicit }}
      toolModelOverrides: {{}}
    oauth: {{ required: false, scopes: [], audience: a2a }}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#
    );
    let boot = init_services_bootstrap(&services);
    let pool = test_db_pool().await;
    let ctx = test_app_context_on(&boot, &pool);
    seed_running_service(&ctx, &name, "agent", port).await?;
    Ok(Agent {
        name,
        ctx,
        _boot: boot,
    })
}

fn extractor(ctx: &AppContext) -> JwtContextExtractor {
    let analytics = Arc::clone(&ctx.analytics_repositories().session_store);
    let user_provider: Arc<dyn UserProvider> =
        Arc::new(UserService::new(Arc::clone(ctx.user_repository())));
    JwtContextExtractor::new(
        analytics,
        user_provider,
        JtiRevocationChecker::from_repository(
            ctx.oauth_repositories().oauth.clone(),
            systemprompt_traits::CacheEpochs::new(),
        ),
        ctx.config().jwt_issuer.clone(),
        systemprompt_test_fixtures::test_signing_keys(),
    )
}

fn agent_route(ctx: &AppContext) -> Router {
    agents::router(ctx)
        .layer(DefaultBodyLimit::max(AGENT_REQUEST_BODY_LIMIT_BYTES))
        .with_auth(
            A2AContextMiddleware::new(
                extractor(ctx),
                AGENT_REQUEST_BODY_LIMIT_BYTES,
                BodyBudget::new(AGENT_BUFFERED_BYTES_BUDGET),
            ),
            AuthzPolicy::authenticated(),
        )
        .layer(axum::Extension(Arc::clone(ctx.config_arc())))
}

fn message_with_file_part(file_bytes: usize) -> String {
    let file = "A".repeat(file_bytes);
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "message/send",
        "params": {
            "message": {
                "role": "user",
                "messageId": "m-1",
                "parts": [
                    { "kind": "file", "file": { "name": "big.bin", "mimeType": "application/octet-stream", "bytes": file } }
                ],
                "contextId": "33333333-3333-4333-8333-333333333333"
            }
        }
    })
    .to_string()
}

#[tokio::test]
async fn a_twenty_mib_a2a_request_with_a_file_part_reaches_the_agent() -> anyhow::Result<()> {
    let backend = MockServer::builder()
        .listener(agent_range_listener()?)
        .start()
        .await;
    let agent = declared_agent(backend.address().port()).await?;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(|req: &MockRequest| {
            ResponseTemplate::new(200).set_body_string(req.body.len().to_string())
        })
        .expect(1)
        .mount(&backend)
        .await;
    let credential = seed_admin_credential(&test_db_pool().await, "agent-body-limit").await?;
    let body = message_with_file_part(FILE_PART_BYTES);
    let sent = body.len();
    assert!(sent > FILE_PART_BYTES && sent < AGENT_REQUEST_BODY_LIMIT_BYTES);

    let resp = agent_route(&agent.ctx)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/{}", agent.name))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", credential.jwt.as_str()),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))?,
        )
        .await?;

    let status = resp.status();
    let echoed = axum::body::to_bytes(resp.into_body(), 1024).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&echoed)
    );
    assert_eq!(
        String::from_utf8_lossy(&echoed),
        sent.to_string(),
        "the backend receives the whole body"
    );
    backend.verify().await;
    Ok(())
}

#[tokio::test]
async fn an_a2a_request_over_the_agent_route_limit_is_a_413() -> anyhow::Result<()> {
    let backend = MockServer::builder()
        .listener(agent_range_listener()?)
        .start()
        .await;
    let agent = declared_agent(backend.address().port()).await?;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&backend)
        .await;
    let credential = seed_admin_credential(&test_db_pool().await, "agent-body-over").await?;

    let resp = agent_route(&agent.ctx)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/{}", agent.name))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", credential.jwt.as_str()),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::CONTENT_LENGTH,
                    (AGENT_REQUEST_BODY_LIMIT_BYTES + 1).to_string(),
                )
                .body(Body::from_stream(futures::stream::pending::<
                    Result<Bytes, Infallible>,
                >()))?,
        )
        .await?;

    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    backend.verify().await;
    Ok(())
}
