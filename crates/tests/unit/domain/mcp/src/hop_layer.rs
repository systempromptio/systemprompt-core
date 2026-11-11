//! The hop-token layer of the MCP router, driven over HTTP: a token the API
//! proxy minted for this server admits the request; a token for another
//! server, an expired one, one signed by an unknown key, one presented with a
//! different MCP session, or a forged value is refused with 401 before the MCP
//! service runs.

use std::sync::Arc;

use axum::body::Body;
use chrono::{Duration, Utc};
use http::StatusCode;
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_mcp::middleware::HopVerifier;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_mcp::{McpHttpConfig, create_router};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_security::jwt::{encode_mcp_hop, mcp_hop_claims};
use systemprompt_security::keys::{RsaSigningKey, SigningKeys};
use systemprompt_test_fixtures::{
    FIXTURE_EXTERNAL_URL, ensure_test_bootstrap, mint_mcp_hop, test_db_pool, test_signing_keys,
    test_token_authority,
};
use tower::ServiceExt;

const SERVER: &str = "hop-layer-srv";

#[derive(Debug, Clone)]
struct NullHandler;

impl rmcp::ServerHandler for NullHandler {}

fn user() -> AuthenticatedUser {
    AuthenticatedUser::new(
        UserId::new(uuid::Uuid::new_v4().to_string()),
        "hop@test.invalid".to_owned(),
        "hop@test.invalid".to_owned(),
        vec![Permission::User],
    )
}

async fn router() -> axum::Router {
    let _ = ensure_test_bootstrap();
    let db = test_db_pool().await;
    create_router(
        NullHandler,
        Arc::new(McpSessionRepository::new(&db)),
        McpHttpConfig::default(),
        HopVerifier::new(
            test_token_authority(FIXTURE_EXTERNAL_URL),
            McpServerId::new(SERVER),
        ),
    )
}

fn initialize(headers: &[(&str, &str)]) -> http::Request<Body> {
    let mut builder = http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "127.0.0.1")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("x-session-id", "s-hop")
        .header("x-trace-id", "t-hop")
        .header("x-agent-name", "agent-hop");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "hop-test", "version": "0"}
        }
    });
    builder
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

async fn status(headers: &[(&str, &str)]) -> StatusCode {
    router()
        .await
        .oneshot(initialize(headers))
        .await
        .expect("router responds")
        .status()
}

fn token_for(server: &str, mcp_session: Option<&str>) -> String {
    mint_mcp_hop(
        &McpServerId::new(server),
        user(),
        mcp_session,
        FIXTURE_EXTERNAL_URL,
    )
}

#[tokio::test]
async fn a_hop_token_minted_for_this_server_is_admitted() {
    let token = token_for(SERVER, None);
    let status = status(&[("x-systemprompt-mcp-hop", &token)]).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_hop_token_for_another_server_is_refused() {
    let token = token_for("another-server", None);
    let status = status(&[("x-systemprompt-mcp-hop", &token)]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_expired_hop_token_is_refused() {
    let mut claims = mcp_hop_claims(
        FIXTURE_EXTERNAL_URL,
        &McpServerId::new(SERVER),
        &systemprompt_models::auth::McpHopIdentity {
            user_id: UserId::new("expired-user"),
            user: None,
            session_id: systemprompt_identifiers::SessionId::new("s"),
            user_type: systemprompt_models::auth::UserType::Anon,
            client_id: None,
            act_chain: Vec::new(),
            purpose: None,
            interaction: None,
            mcp_session: None,
        },
    );
    let past = (Utc::now() - Duration::minutes(5)).timestamp();
    claims.registered.iat = past - 60;
    claims.registered.nbf = Some(past - 60);
    claims.registered.exp = past;
    let token = encode_mcp_hop(&test_signing_keys(), &claims).expect("encode");
    let status = status(&[("x-systemprompt-mcp-hop", &token)]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_hop_token_signed_by_an_unknown_key_is_refused() {
    let foreign =
        SigningKeys::new(RsaSigningKey::generate().expect("key"), &[3; 32]).expect("keys");
    let claims = mcp_hop_claims(
        FIXTURE_EXTERNAL_URL,
        &McpServerId::new(SERVER),
        &systemprompt_models::auth::McpHopIdentity {
            user_id: UserId::new("foreign-user"),
            user: None,
            session_id: systemprompt_identifiers::SessionId::new("s"),
            user_type: systemprompt_models::auth::UserType::Anon,
            client_id: None,
            act_chain: Vec::new(),
            purpose: None,
            interaction: None,
            mcp_session: None,
        },
    );
    let token = encode_mcp_hop(&foreign, &claims).expect("encode");
    let status = status(&[("x-systemprompt-mcp-hop", &token)]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_hop_token_presented_with_another_mcp_session_is_refused() {
    let token = token_for(SERVER, Some("sess-bound"));
    let status = status(&[
        ("x-systemprompt-mcp-hop", &token),
        ("mcp-session-id", "sess-other"),
    ])
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_forged_hop_value_is_refused() {
    let status = status(&[
        ("x-systemprompt-mcp-hop", "true"),
        ("x-proxy-verified", "true"),
        ("x-user-id", "forged"),
    ])
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
