//! Managed-MCP proxy edge: every request is authenticated on its own
//! credential, a session id is bound to its server and principal, the proxy
//! owns the identity headers the backend sees, and the request body is capped.

use axum::body::Body;
use axum::http::Request;
use systemprompt_api::routes::proxy::mcp;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, JwtToken, McpServerId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::repository::McpProxySessionRepository;
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{
    TestBootstrap, init_services_bootstrap, mint_admin_jwt, seed_running_service, seed_user_row,
    test_app_context_on, test_config, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::body_to_string;

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
const LIST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

struct Edge {
    name: String,
    backend: MockServer,
    ctx: std::sync::Arc<systemprompt_runtime::AppContext>,
    pool: systemprompt_database::DbPool,
    _boot: TestBootstrap,
}

struct Caller {
    user: UserId,
    token: String,
}

async fn caller(pool: &systemprompt_database::DbPool) -> Caller {
    let user = UserId::new(Uuid::new_v4().to_string());
    seed_user_row(
        pool,
        &user,
        &format!("{}@proxy-edge.invalid", user.as_str()),
    )
    .await
    .expect("seed caller");
    let token = mint_admin_jwt(&user, "proxy-edge@test.invalid", &test_config().jwt_issuer)
        .as_str()
        .to_owned();
    Caller { user, token }
}

async fn edge(issued_session: &str) -> anyhow::Result<Edge> {
    let listener = (5000..=5999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an allowed MCP port is available");
    let backend = MockServer::builder().listener(listener).start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", issued_session)
                .set_body_json(serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}})),
        )
        .mount(&backend)
        .await;
    let name = format!("edge_mcp_{}", Uuid::new_v4().simple());
    let services = format!(
        r#"mcp_servers:
  {name}:
    server_type: internal
    binary: fixture
    package: fixture
    port: {}
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
"#,
        backend.address().port()
    );
    let boot = init_services_bootstrap(&services);
    let manifest_dir = boot.system_path.join("extensions").join("fixture");
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("manifest.yaml"),
        "extension:\n  type: mcp\n  name: fixture\n  binary: fixture\n",
    )?;
    let pool = test_db_pool().await;
    let ctx = test_app_context_on(&boot, &pool);
    seed_running_service(&ctx, &name, "mcp", backend.address().port()).await?;
    Ok(Edge {
        name,
        backend,
        ctx,
        pool,
        _boot: boot,
    })
}

fn request(
    edge: &Edge,
    who: Option<&Caller>,
    session: Option<&str>,
    body: Body,
) -> anyhow::Result<Request<Body>> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/{}/mcp", edge.name))
        .header("content-type", "application/json")
        .header("x-systemprompt-mcp-hop", "forged")
        .header("x-proxy-verified", "true")
        .header("x-user-permissions", "admin")
        .header("x-user-roles", "admin")
        .header("x-user-id", "forged-user");
    if let Some(caller) = who {
        builder = builder.header("authorization", format!("Bearer {}", caller.token));
    }
    if let Some(session) = session {
        builder = builder.header("mcp-session-id", session);
    }
    let mut request = builder.body(body)?;
    let actor_user = who.map_or_else(UserId::generate, |caller| caller.user.clone());
    let mut rc = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::system(),
        Actor::user(actor_user),
    );
    if let Some(caller) = who {
        rc = rc.with_auth_token(JwtToken::new(caller.token.clone()));
    }
    request.extensions_mut().insert(rc);
    Ok(request)
}

async fn send(edge: &Edge, request: Request<Body>) -> anyhow::Result<(http::StatusCode, String)> {
    let response = super::common::with_config(mcp::router(&edge.ctx), &edge.ctx)
        .oneshot(request)
        .await?;
    body_to_string(response).await
}

async fn upstream_count(edge: &Edge) -> usize {
    edge.backend
        .received_requests()
        .await
        .expect("wiremock request recording")
        .len()
}

fn issued() -> String {
    format!("edge-{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn a_request_carrying_only_a_session_id_is_challenged() -> anyhow::Result<()> {
    let session = issued();
    let edge = edge(&session).await?;
    McpProxySessionRepository::new(&edge.pool)
        .bind(
            &SessionId::new(session.as_str()),
            &McpServerId::new(edge.name.as_str()),
            Some(&caller(&edge.pool).await.user),
        )
        .await?;

    let (status, body) = send(
        &edge,
        request(&edge, None, Some(&session), Body::from(LIST))?,
    )
    .await?;

    assert_eq!(status, http::StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(upstream_count(&edge).await, 0);
    Ok(())
}

#[tokio::test]
async fn an_authenticated_session_flow_reaches_the_backend_with_proxy_owned_identity()
-> anyhow::Result<()> {
    let session = issued();
    let edge = edge(&session).await?;
    let alice = caller(&edge.pool).await;

    let (status, body) = send(&edge, request(&edge, Some(&alice), None, Body::from(INIT))?).await?;
    assert_eq!(status, http::StatusCode::OK, "{body}");
    let (status, body) = send(
        &edge,
        request(&edge, Some(&alice), Some(&session), Body::from(LIST))?,
    )
    .await?;
    assert_eq!(status, http::StatusCode::OK, "{body}");

    let received = edge.backend.received_requests().await.expect("recording");
    assert_eq!(received.len(), 2);
    let server = McpServerId::new(edge.name.as_str());
    let tokens = edge.ctx.token_authority();
    let mut hop_sessions = Vec::new();
    for upstream in &received {
        let header = |name: &str| upstream.headers.get(name).and_then(|v| v.to_str().ok());
        assert_eq!(header("x-proxy-verified"), None);
        assert_eq!(header("x-user-permissions"), None);
        assert_eq!(header("x-user-roles"), None);
        assert_eq!(header("x-user-id"), None);
        assert_eq!(
            header("authorization"),
            Some(format!("Bearer {}", alice.token).as_str())
        );
        let hop =
            header("x-systemprompt-mcp-hop").expect("every forwarded request carries a hop token");
        let claims = tokens
            .verify_mcp_hop(hop, &server)
            .expect("the hop token verifies for this server");
        assert_eq!(claims.identity.user_id, alice.user);
        assert_eq!(
            claims.identity.user.as_ref().map(|user| user.id.clone()),
            Some(alice.user.clone())
        );
        hop_sessions.push(claims.identity.mcp_session);
    }
    assert_eq!(hop_sessions, vec![None, Some(session.clone())]);
    assert_eq!(
        received[1]
            .headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok()),
        Some(session.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn another_principal_presenting_a_bound_session_gets_not_found() -> anyhow::Result<()> {
    let session = issued();
    let edge = edge(&session).await?;
    let alice = caller(&edge.pool).await;
    let bob = caller(&edge.pool).await;
    let (status, _) = send(&edge, request(&edge, Some(&alice), None, Body::from(INIT))?).await?;
    assert_eq!(status, http::StatusCode::OK);

    let (status, body) = send(
        &edge,
        request(&edge, Some(&bob), Some(&session), Body::from(LIST))?,
    )
    .await?;

    assert_eq!(status, http::StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("session_not_found"), "{body}");
    assert_eq!(upstream_count(&edge).await, 1);
    Ok(())
}

#[tokio::test]
async fn a_session_bound_to_another_server_gets_not_found() -> anyhow::Result<()> {
    let session = issued();
    let edge = edge(&session).await?;
    let alice = caller(&edge.pool).await;
    McpProxySessionRepository::new(&edge.pool)
        .bind(
            &SessionId::new(session.as_str()),
            &McpServerId::new(format!("other_{}", Uuid::new_v4().simple())),
            Some(&alice.user),
        )
        .await?;

    let (status, body) = send(
        &edge,
        request(&edge, Some(&alice), Some(&session), Body::from(LIST))?,
    )
    .await?;

    assert_eq!(status, http::StatusCode::NOT_FOUND, "{body}");
    assert_eq!(upstream_count(&edge).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_declared_oversize_body_is_refused_before_it_is_read() -> anyhow::Result<()> {
    let edge = edge(&issued()).await?;
    let alice = caller(&edge.pool).await;
    let mut oversize = request(&edge, Some(&alice), None, Body::empty())?;
    oversize
        .headers_mut()
        .insert("content-length", http::HeaderValue::from_static("8388608"));

    let (status, body) = send(&edge, oversize).await?;

    assert_eq!(status, http::StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    assert_eq!(upstream_count(&edge).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_streamed_body_past_the_cap_is_refused() -> anyhow::Result<()> {
    let edge = edge(&issued()).await?;
    let alice = caller(&edge.pool).await;
    let chunks =
        (0..9).map(|_| Ok::<_, std::io::Error>(bytes::Bytes::from(vec![b' '; 512 * 1024])));
    let body = Body::from_stream(futures::stream::iter(chunks));

    let (status, body) = send(&edge, request(&edge, Some(&alice), None, body)?).await?;

    assert_eq!(status, http::StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    Ok(())
}
