//! The MCP serving framework of the toolset host.
//!
//! [`create_router`] wraps an rmcp server in rmcp's streamable-HTTP service
//! behind the hop layer ([`middleware::HopVerifier`]): every request must
//! carry the API proxy's hop token for this server, verified against the
//! host's public key set. Sessions are mirrored into the session store the
//! `mcp` member provides (`DynMcpSessionStore`, sealed) so a session survives
//! a restart, and a session is restored only for the server and verified user
//! it is bound to. [`bind_loopback`] binds the listener on 127.0.0.1 only.
//! [`host`] is the generic toolset server; [`middleware`] holds the hop, RBAC
//! and session layers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod host;
pub mod middleware;

use std::time::Duration;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use rmcp::ServerHandler;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use systemprompt_extension::mcp::DynMcpSessionStore;
use systemprompt_extension::mcp::capabilities::MAX_REQUEST_BODY_BYTES;

use middleware::DatabaseSessionHandler;

#[derive(Debug, Clone)]
pub struct McpHttpConfig {
    pub allowed_hosts: Option<Vec<String>>,
    pub allowed_origins: Vec<String>,
    pub session: SessionTimeouts,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SessionTimeouts {
    pub init: Option<Duration>,
    pub keep_alive: Option<Duration>,
}

impl Default for McpHttpConfig {
    fn default() -> Self {
        Self {
            // Why: rmcp's `host_is_allowed` treats port-less entries as matching any port.
            allowed_hosts: Some(vec![
                "localhost".into(),
                "127.0.0.1".into(),
                "0.0.0.0".into(),
                "::1".into(),
                "::".into(),
            ]),
            allowed_origins: Vec::new(),
            session: SessionTimeouts::default(),
        }
    }
}

pub async fn bind_loopback(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(std::net::SocketAddr::from((
        std::net::Ipv4Addr::LOCALHOST,
        port,
    )))
    .await
}

async fn mcp_request_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let session_id = req
        .headers()
        .get(systemprompt_identifiers::headers::MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let mcp_method = req
        .headers()
        .get(systemprompt_identifiers::headers::MCP_METHOD)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let has_hop = req.extensions().get::<middleware::VerifiedHop>().is_some();

    tracing::info!(
        %method,
        %uri,
        session_id = ?session_id,
        mcp_method = ?mcp_method,
        has_hop,
        "MCP request received"
    );

    let response = next.run(req).await;

    let status = response.status();
    if !status.is_success() {
        tracing::error!(
            %method,
            %uri,
            session_id = ?session_id,
            status = %status,
            "MCP request failed at transport level"
        );
    }

    response
}

pub fn create_router<S>(
    server: S,
    sessions: DynMcpSessionStore,
    http: McpHttpConfig,
    hop: middleware::HopVerifier,
) -> axum::Router
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let McpHttpConfig {
        allowed_hosts,
        allowed_origins,
        session,
    } = http;
    let server_id = hop.server_id().clone();

    let host_policy = StreamableHttpServerConfig::default().with_allowed_origins(allowed_origins);
    let host_policy = match allowed_hosts {
        Some(hosts) => host_policy.with_allowed_hosts(hosts),
        None => host_policy.disable_allowed_hosts(),
    };
    let mut config = host_policy
        .with_sse_keep_alive(session.keep_alive)
        .with_max_request_body_bytes(MAX_REQUEST_BODY_BYTES);
    let session_store: std::sync::Arc<
        dyn rmcp::transport::streamable_http_server::session::store::SessionStore,
    > = std::sync::Arc::new(middleware::PostgresSessionStore::new(
        std::sync::Arc::clone(&sessions),
    ));
    config.session_store = Some(session_store);

    let session_handler = DatabaseSessionHandler::with_timeouts(sessions, session, Some(server_id));

    let service =
        StreamableHttpService::new(move || Ok(server.clone()), session_handler.into(), config);

    axum::Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(mcp_request_logger))
        .layer(axum::middleware::from_fn_with_state(
            hop,
            middleware::verify_hop,
        ))
        .layer(axum::middleware::map_response(
            |mut response: http::Response<_>| async move {
                response
                    .headers_mut()
                    .insert("x-accel-buffering", http::HeaderValue::from_static("no"));
                response
            },
        ))
}
