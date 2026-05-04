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
//! - [`services::McpManager`], [`services::orchestrator::McpOrchestrator`] —
//!   top-level service supervisors.
//! - [`services::registry::RegistryManager`] — registry of MCP servers
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
//! `anyhow`) are composed via `#[from]` on the error enum.

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

/// Wire-protocol version implemented by this crate's MCP server runtime.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Convenience alias for results returned from MCP request handlers.
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
pub use services::{EventBus as McpEventBus, McpEvent, McpManager, ServiceManager};

pub use orchestration::{
    McpServerConnectionInfo, McpServerMetadata, McpServiceState, McpToolLoader, ServerStatus,
    ServiceStateManager, SkillLoadingResult,
};

pub use systemprompt_models::mcp::{
    DynMcpDeploymentProvider, DynMcpRegistry, DynMcpToolProvider, McpDeploymentProvider,
    McpProvider, McpRegistry, McpServerState,
};

/// Returns the `rmcp` runtime's currently-advertised protocol version string.
pub fn mcp_protocol_version() -> String {
    ProtocolVersion::LATEST.to_string()
}

/// Public re-export of the registry surface.
pub mod registry {
    pub use crate::services::registry::RegistryManager;
}

pub use cli::{list_services, show_status, start_services, stop_services};

pub(crate) mod state;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use rmcp::ServerHandler;
pub use rmcp::model::ProtocolVersion;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use systemprompt_database::DbPool;

use crate::middleware::DatabaseSessionManager;

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

/// Build an axum router that mounts the MCP streamable-HTTP service at `/mcp`,
/// with request logging and SSE-buffer-disable middleware applied.
pub fn create_router<S>(server: S, db_pool: &DbPool) -> axum::Router
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let config = StreamableHttpServerConfig::default();

    let session_manager = DatabaseSessionManager::new(db_pool);

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
