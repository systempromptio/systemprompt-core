//! Native Model Context Protocol (MCP) implementation for systemprompt.io.
//!
//! This crate hosts the in-process MCP server runtime, the
//! registry/orchestrator that supervises per-tenant MCP child processes, the
//! OAuth2/RBAC middleware, and the tool/resource/UI-renderer abstractions used
//! across the platform.
//!
//! # Layered components
//!
//! - [`extension::McpExtension`] — `Extension` registration entry-point.
//! - [`services::McpOrchestrator`] — top-level service supervisor.
//! - [`services::registry::RegistryService`] — registry of MCP servers
//!   configured via `services.yaml`.
//! - [`services::tool_provider::McpToolProvider`] — tool-discovery + execution
//!   facade.
//! - [`middleware::rbac`] — JWT/proxy-verified RBAC layer.
//! - [`orchestration`] — multi-server lifecycle/state management.
//! - [`repository`] — Postgres persistence for sessions, artifacts, tool usage.
//!
//! # Feature matrix
//!
//! This crate has no Cargo features today; it is built as a single unit. The
//! facade crate `systemprompt` gates this crate behind the `mcp` / `full`
//! features.
//!
//! # Errors
//!
//! All public APIs return [`McpDomainResult`] — a typed `Result` aliased over
//! [`McpDomainError`]. External error types (`sqlx`, `serde_json`, `io`,
//! join errors) are composed via `#[from]` on the error enum. Third-party
//! errors without a typed adapter are converted at the boundary with
//! `.map_err(|e| McpDomainError::Internal(e.to_string()))`.

pub(crate) mod capabilities;
pub(crate) mod cli;
pub(crate) mod error;
pub(crate) mod extension;
pub(crate) mod jobs;
pub mod middleware;
pub mod models;
pub mod orchestration;
pub(crate) mod progress;
pub mod repository;
pub(crate) mod resources;
pub(crate) mod response;
pub(crate) mod schema;
pub mod services;
pub(crate) mod tool;

pub use extension::McpExtension;

pub use error::{McpDomainError, McpDomainResult};
pub use rmcp::ErrorData as McpError;

pub type McpResult<T> = Result<T, McpError>;

pub use capabilities::{
    WEBSITE_URL, build_experimental_capabilities, default_tool_visibility, mcp_apps_ui_extension,
    model_only_visibility, tool_ui_meta, visibility_to_json,
};
pub use progress::{ProgressCallback, create_progress_callback};
pub use repository::{CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository};
pub use resources::{
    ArtifactViewerConfig, build_artifact_viewer_resource, default_server_icons,
    read_artifact_viewer_resource,
};
pub use response::McpResponseBuilder;
pub use schema::McpOutputSchema;
pub use tool::{McpToolExecutor, McpToolHandler};

pub use systemprompt_models::mcp::{
    Deployment, DeploymentConfig, ERROR, McpAuthState, McpServerConfig, OAuthRequirement, RUNNING,
    STARTING, STOPPED, Settings,
};

pub use services::monitoring::health::HealthStatus;
pub use services::registry::McpServerRegistry;
pub use services::registry::trait_impl::McpDeploymentProviderImpl;
pub use services::tool_provider::McpToolProvider;
pub use services::{EventBus as McpEventBus, McpEvent, McpOrchestrator};

pub use orchestration::{
    McpServerConnectionInfo, McpServerMetadata, McpServiceState, McpToolLoader, ServerStatus,
    ServiceStateService, SkillLoadingResult,
};

pub use systemprompt_models::mcp::{
    DynMcpDeploymentProvider, DynMcpRegistry, DynMcpToolProvider, McpDeploymentProvider,
    McpProvider, McpRegistry, McpServerState,
};

pub fn mcp_protocol_version() -> String {
    ProtocolVersion::LATEST.to_string()
}

pub fn mcp_protocol_version_str() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(mcp_protocol_version).as_str()
}

pub mod registry {
    pub use crate::services::registry::RegistryService;
}

pub use cli::{list_services, show_status, start_services, stop_services};

pub(crate) mod state;

use std::time::Duration;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use rmcp::ServerHandler;
pub use rmcp::model::ProtocolVersion;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use systemprompt_database::DbPool;

use crate::middleware::DatabaseSessionHandler;

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
            // Why: `0.0.0.0` and `[::]` are common bind addresses for the
            // local MCP server; clients connecting via the bind URL send a
            // matching `Host` header that the default allow-list must
            // accept. Port-less entries match any port via rmcp's
            // `host_is_allowed`.
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

pub use state::McpState;

async fn mcp_request_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let session_id = req
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let has_auth = req.headers().get("authorization").is_some();
    let proxy_verified = req
        .headers()
        .get("x-proxy-verified")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "true");
    let accept = req
        .headers()
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    tracing::info!(
        %method,
        %uri,
        session_id = ?session_id,
        has_auth,
        proxy_verified,
        accept = ?accept,
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

pub fn create_router<S>(server: S, db_pool: &DbPool, http: McpHttpConfig) -> axum::Router
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let McpHttpConfig {
        allowed_hosts,
        allowed_origins,
        session,
    } = http;

    let host_policy = StreamableHttpServerConfig::default().with_allowed_origins(allowed_origins);
    let host_policy = match allowed_hosts {
        Some(hosts) => host_policy.with_allowed_hosts(hosts),
        None => host_policy.disable_allowed_hosts(),
    };
    let config = host_policy.with_sse_keep_alive(session.keep_alive);

    let session_manager = DatabaseSessionHandler::with_timeouts(db_pool, session);

    let service =
        StreamableHttpService::new(move || Ok(server.clone()), session_manager.into(), config);

    axum::Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(mcp_request_logger))
        .layer(axum::middleware::map_response(
            |mut response: http::Response<_>| async move {
                response
                    .headers_mut()
                    .insert("x-accel-buffering", http::HeaderValue::from_static("no"));
                response
            },
        ))
}
