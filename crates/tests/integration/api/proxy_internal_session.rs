//! Full internal-MCP proxy path for a session-only follow-up request.

use axum::body::Body;
use axum::http::Request;
use systemprompt_api::routes::proxy::mcp;
use systemprompt_identifiers::{AgentName, ContextId, JwtToken, SessionId, TraceId, UserId};
use systemprompt_mcp::repository::{McpProxyIdentityRepository, ProxyIdentityRow};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{Permission, UserType};
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_db_pool, init_services_bootstrap, seed_running_service,
    seed_user_row,
};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};

use super::common::body_to_string;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn internal_mcp_followup_restores_cached_identity_before_forwarding() -> anyhow::Result<()> {
    let listener = (5000..=5999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an allowed MCP port is available");
    let backend = MockServer::builder().listener(listener).start().await;
    let name = format!("session_mcp_{}", Uuid::new_v4().simple());
    let services = format!(
        r#"mcp_servers:
  {name}:
    type: internal
    binary: fixture
    package: fixture
    port: {}
    enabled: true
    tool_policy: allow
    display_in_web: false
    oauth:
      required: false
      scopes: []
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
    let pool = fixture_db_pool(&boot.database_url).await?;
    let ctx = fixture_app_context(&pool, &boot.database_url)?;
    seed_running_service(&pool, &name, "mcp", backend.address().port()).await?;

    let session_id = SessionId::new(format!("followup-{}", Uuid::new_v4().simple()));
    let user_id = UserId::new(Uuid::new_v4().to_string());
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@proxy.invalid", user_id.as_str()),
    )
    .await?;
    McpProxyIdentityRepository::new(&pool)?
        .upsert(
            &session_id,
            &ProxyIdentityRow {
                user_id: user_id.clone(),
                user_type: UserType::User,
                permissions: vec![Permission::User],
                roles: vec!["session-fixture".to_owned()],
                auth_token: JwtToken::new("cached-session-token"),
            },
        )
        .await?;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", "Bearer cached-session-token"))
        .and(header("mcp-session-id", session_id.as_str()))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({"identity": "restored"})),
        )
        .expect(1)
        .mount(&backend)
        .await;

    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/{name}/mcp"))
        .header("content-type", "application/json")
        .header("mcp-session-id", session_id.as_str())
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ))?;
    request.extensions_mut().insert(RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::system(),
    ));

    let response = mcp::router(&ctx).oneshot(request).await?;
    let (status, body) = body_to_string(response).await?;
    assert_eq!(status, http::StatusCode::OK, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body)?,
        serde_json::json!({"identity": "restored"})
    );
    let received = backend
        .received_requests()
        .await
        .expect("wiremock request recording");
    assert_eq!(received.len(), 1);
    Ok(())
}
