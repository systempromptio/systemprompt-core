//! Builds and serves one YAML-declared MCP server from its [`HostContext`].
//!
//! [`build_server`] resolves the deployment's enabled `toolsets:` against the
//! built [`Toolsets`] (an id no admitted member contributes fails start),
//! builds each toolset, takes the built access recorders and the MCP member's tool execution for
//! the server and
//! hands the lot to [`ToolsetServer`], which refuses a `tools:` entry no
//! toolset serves.
//! [`serve`] binds the deployment's `port:` on the loopback interface only
//! (the API proxy on the same host is its one caller, and every request must
//! carry the proxy's hop token, verified against the context's public key
//! set) and serves until the listener closes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::contract::AccessRecorders;
use systemprompt_mcp::host::{
    ToolsetAuthz, ToolsetHostError, ToolsetServer, ToolsetServerParts, serve_router,
};
use systemprompt_mcp::middleware::HopVerifier;
use systemprompt_extension::mcp::{DynToolExecutions, McpToolset};
use systemprompt_mcp::{McpHttpConfig, create_router};
use systemprompt_provider_contracts::MissingDependency;
use tokio::net::TcpListener;

use super::factory::ToolsetBuildError;
use crate::HostContext;
use crate::contract::Toolsets;

#[derive(Debug, thiserror::Error)]
pub enum McpHostError {
    #[error("MCP server '{server}' is not declared in the services configuration")]
    UnknownServer { server: McpServerId },
    #[error(
        "MCP server '{server}' lists toolset '{toolset}', which no composed member contributes \
         (contributed: {registered})"
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
    #[error("the host's composed state lacks a built slot")]
    State(#[from] MissingDependency),
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
    ctx: &HostContext,
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

    let state = ctx.state();
    let built = Arc::clone(state.get::<Arc<Toolsets>>()?);
    let recorders = state.get::<Arc<AccessRecorders>>()?.to_vec();

    let mut toolsets: Vec<Box<dyn McpToolset>> = Vec::new();
    for reference in deployment.enabled_toolsets() {
        let factory = built
            .find(&reference.id)
            .ok_or_else(|| McpHostError::UnknownToolset {
                server: server_id.clone(),
                toolset: reference.id.clone(),
                registered: built.ids().join(", "),
            })?;
        match factory(ctx, server_id) {
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

    let executor = state.get::<DynToolExecutions>()?.for_server(server_id.clone());
    ToolsetServer::new(ToolsetServerParts {
        server_id: server_id.clone(),
        executor,
        authz: ToolsetAuthz {
            hook: Arc::clone(ctx.authz_hook()),
            services,
            subjects: ctx.subject_providers().clone(),
        },
        recorders,
        toolsets,
    })
    .map_err(McpHostError::from)
}

pub async fn serve(ctx: Arc<HostContext>, server_id: McpServerId) -> Result<(), McpHostError> {
    let port = resolve_port(&ctx, &server_id)?;
    let listener = systemprompt_mcp::bind_loopback(port)
        .await
        .map_err(|source| McpHostError::Bind {
            addr: format!("127.0.0.1:{port}"),
            source,
        })?;
    serve_on(ctx, server_id, listener).await
}

pub async fn serve_on(
    ctx: Arc<HostContext>,
    server_id: McpServerId,
    listener: TcpListener,
) -> Result<(), McpHostError> {
    let server = build_server(&ctx, &server_id)?;
    let requires_oauth = ctx
        .services_state()
        .view()
        .config()
        .mcp_servers
        .get(server_id.as_str())
        .is_some_and(|deployment| deployment.oauth.required);
    let router = create_router(
        server,
        Arc::clone(ctx.mcp_session_repository()),
        McpHttpConfig::default(),
        HopVerifier::new(
            ctx.verification_keys().clone(),
            &ctx.config().jwt_issuer,
            server_id.clone(),
        )
        .requiring_hop(requires_oauth),
    );
    tracing::info!(server = %server_id, addr = ?listener.local_addr().ok(), "MCP toolset host listening");
    serve_router(listener, router)
        .await
        .map_err(McpHostError::Serve)
}

fn resolve_port(ctx: &HostContext, server_id: &McpServerId) -> Result<u16, McpHostError> {
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
