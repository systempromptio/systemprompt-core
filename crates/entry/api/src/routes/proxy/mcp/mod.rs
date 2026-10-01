//! MCP reverse-proxy routes: forwards requests to managed MCP backends and
//! exposes tool-execution lookups. Per-service discovery metadata lives in the
//! `discovery` submodule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;
mod executions;

pub use executions::{ExecutionsState, ToolExecutionResponse, executions_router};

use crate::services::proxy::ProxyEngine;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{any, get};
use systemprompt_identifiers::McpServerId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::McpRegistryProvider;

#[derive(Clone, Debug)]
pub struct McpState {
    pub ctx: AppContext,
}

pub(in crate::routes) async fn get_mcp_server_scopes(
    registry: &dyn McpRegistryProvider,
    server: &McpServerId,
) -> Option<Vec<String>> {
    match registry.get_server(server.as_str()).await {
        Ok(server_info) if server_info.oauth.required => {
            let scopes: Vec<String> = server_info
                .oauth
                .scopes
                .iter()
                .map(ToString::to_string)
                .collect();
            if scopes.is_empty() {
                None
            } else {
                Some(scopes)
            }
        },
        _ => None,
    }
}

pub(in crate::routes) async fn get_mcp_server_scopes_from_resource(
    registry: &dyn McpRegistryProvider,
    resource_uri: &str,
) -> Option<Vec<String>> {
    let url = reqwest::Url::parse(resource_uri).ok()?;
    let path = url.path();
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 6 || parts[1] != "api" || parts[3] != "mcp" || parts[5] != "mcp" {
        return None;
    }
    let server = McpServerId::try_new(parts[4]).ok()?;
    get_mcp_server_scopes(registry, &server).await
}

pub fn router(ctx: &AppContext) -> Router {
    let repo = crate::repository::tool_usage(ctx.db_pool());
    let identities = crate::repository::proxy_identities(ctx.db_pool());
    let engine = ProxyEngine::new(identities)
        .with_tool_usage_repo(repo, ctx.tool_call_intents())
        .with_artifact_ingest(ctx.artifact_ingest_arc());

    let state = McpState { ctx: ctx.clone() };

    Router::new()
        .route(
            "/{service_name}/mcp/.well-known/oauth-protected-resource",
            get(discovery::handle_mcp_protected_resource),
        )
        .route(
            "/{service_name}/mcp/.well-known/oauth-authorization-server",
            get(discovery::handle_mcp_authorization_server),
        )
        .route(
            "/{service_name}/{*path}",
            any({
                let ctx_clone = ctx.clone();
                move |Path((service_name, path)): Path<(String, String)>, request| {
                    let engine = engine.clone();
                    let ctx = ctx_clone.clone();
                    async move {
                        engine
                            .handle_mcp_request_with_path(
                                Path((service_name, path)),
                                State(ctx),
                                request,
                            )
                            .await
                    }
                }
            }),
        )
        .with_state(state)
}
