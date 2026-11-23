//! MCP reverse-proxy routes: forwards requests to managed MCP backends and
//! exposes tool-execution lookups. Per-service discovery metadata lives in the
//! `discovery` submodule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

mod discovery;
mod executions;

pub use executions::{ExecutionsState, ToolExecutionResponse, executions_router};

use crate::kernel::ProxyService;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{any, get};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::AppContext;

#[derive(Clone, Debug)]
pub struct McpState {
    pub ctx: AppContext,
}

pub fn router(ctx: &AppContext) -> Result<Router, MissingDependency> {
    let repo = crate::repository::tool_usage(ctx.db_pool());
    let sessions = crate::repository::proxy_sessions(ctx.db_pool());
    let engine = ProxyService::new(
        sessions,
        crate::services::kernel_seeds::connector_tokens(ctx)?,
        &ctx.services_state().view(),
        Arc::clone(ctx.config_arc()),
        Arc::clone(ctx.secrets()),
    );
    let engine = match ctx
        .kernel()
        .state()
        .find::<systemprompt_traits::DynToolCallIntentClaims>()
    {
        Some(intents) => engine.with_tool_usage_repo(repo, Arc::clone(intents)),
        None => engine,
    };
    let engine = match ctx
        .kernel()
        .state()
        .get::<Arc<systemprompt_mcp::ArtifactIngest>>()
    {
        Ok(ingest) => engine.with_artifact_ingest(Arc::clone(ingest)),
        Err(_) => engine,
    };

    let state = McpState { ctx: ctx.clone() };

    Ok(Router::new()
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
        .with_state(state))
}
