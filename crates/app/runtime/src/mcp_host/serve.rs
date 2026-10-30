//! Builds and serves one YAML-declared MCP server from the application
//! context.
//!
//! [`build_server`] resolves the deployment's enabled `toolsets:` against the
//! factory inventory (an id no linked crate registers fails start), builds
//! each toolset, discovers the registered access recorders and hands the lot
//! to [`ToolsetServer`], which refuses a `tools:` entry no toolset serves.
//! [`serve`] binds the deployment's `port:` and serves until the
//! listener closes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::host::{
    ToolsetAuthz, ToolsetHostError, ToolsetServer, ToolsetServerParts,
    discover_mcp_access_recorders, serve_router,
};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::{McpHttpConfig, McpToolExecutor, McpToolset, create_router};
use tokio::net::TcpListener;

use super::factory::{ToolsetBuildError, find_toolset_factory, registered_toolset_ids};
use crate::AppContext;

#[derive(Debug, thiserror::Error)]
pub enum McpHostError {
    #[error("MCP server '{server}' is not declared in the services configuration")]
    UnknownServer { server: McpServerId },
    #[error(
        "MCP server '{server}' lists toolset '{toolset}', which no linked crate registers \
         (registered: {registered})"
    )]
    UnknownToolset {
        server: McpServerId,
        toolset: String,
        registered: String,
    },
    #[error("MCP server '{server}': toolset '{toolset}' could not be built")]
    Build {
        server: McpServerId,
        toolset: String,
        #[source]
        source: ToolsetBuildError,
    },
    #[error(transparent)]
    Host(#[from] ToolsetHostError),
    #[error("MCP server '{server}' has no `port:` in its YAML")]
    NoPort { server: McpServerId },
    #[error("failed to bind {addr}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("MCP server stopped with an I/O error")]
    Serve(#[source] std::io::Error),
}

pub fn build_server(
    ctx: &AppContext,
    server_id: &McpServerId,
) -> Result<ToolsetServer, McpHostError> {
    let services = ctx.services_state().view();
    let config = services.config();
    let deployment =
        config
            .mcp_servers
            .get(server_id.as_str())
            .ok_or_else(|| McpHostError::UnknownServer {
                server: server_id.clone(),
            })?;

    let mut toolsets: Vec<Box<dyn McpToolset>> = Vec::new();
    for reference in deployment.enabled_toolsets() {
        let factory =
            find_toolset_factory(&reference.id).ok_or_else(|| McpHostError::UnknownToolset {
                server: server_id.clone(),
                toolset: reference.id.clone(),
                registered: registered_toolset_ids().join(", "),
            })?;
        match (factory.build)(ctx, server_id) {
            Ok(Some(toolset)) => toolsets.push(toolset),
            Ok(None) => {
                tracing::warn!(server = %server_id, toolset = %reference.id, "toolset unavailable in this installation; not served");
            },
            Err(source) => {
                return Err(McpHostError::Build {
                    server: server_id.clone(),
                    toolset: reference.id.clone(),
                    source,
                });
            },
        }
    }

    let executor = McpToolExecutor::new(
        Arc::new(ToolUsageRepository::new(ctx.db_pool())),
        ctx.tool_call_intents(),
        ctx.artifact_ingest_arc(),
        server_id.clone(),
    );
    ToolsetServer::new(ToolsetServerParts {
        server_id: server_id.clone(),
        executor,
        authz: ToolsetAuthz {
            hook: Arc::clone(ctx.authz_hook()),
            services,
            tokens: ctx.token_authority(),
            config: Arc::clone(ctx.config_arc()),
            subjects: ctx.subject_providers().clone(),
        },
        recorders: discover_mcp_access_recorders(ctx.db_pool()),
        toolsets,
    })
    .map_err(McpHostError::from)
}

pub async fn serve(ctx: Arc<AppContext>, server_id: McpServerId) -> Result<(), McpHostError> {
    let port = resolve_port(&ctx, &server_id)?;
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|source| McpHostError::Bind { addr, source })?;
    serve_on(ctx, server_id, listener).await
}

pub async fn serve_on(
    ctx: Arc<AppContext>,
    server_id: McpServerId,
    listener: TcpListener,
) -> Result<(), McpHostError> {
    let server = build_server(&ctx, &server_id)?;
    let router = create_router(
        server,
        Arc::clone(ctx.mcp_session_repository()),
        McpHttpConfig {
            server_id: Some(server_id.clone()),
            ..McpHttpConfig::default()
        },
    );
    tracing::info!(server = %server_id, addr = ?listener.local_addr().ok(), "MCP toolset host listening");
    serve_router(listener, router)
        .await
        .map_err(McpHostError::Serve)
}

fn resolve_port(ctx: &AppContext, server_id: &McpServerId) -> Result<u16, McpHostError> {
    ctx.services_state()
        .view()
        .config()
        .mcp_servers
        .get(server_id.as_str())
        .and_then(|deployment| deployment.port)
        .ok_or_else(|| McpHostError::NoPort {
            server: server_id.clone(),
        })
}
