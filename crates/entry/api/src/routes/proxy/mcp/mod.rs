//! MCP reverse-proxy routes: forwards requests to managed MCP backends.
//! Per-service discovery metadata lives in the `discovery` submodule; the
//! tool-execution lookup is the `mcp-http` member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

mod discovery;

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
    let state = ctx.kernel().state();
    let intent_claims = state
        .find::<systemprompt_traits::DynToolCallIntentClaims>()
        .cloned();
    if intent_claims.is_none() {
        tracing::info!(
            "the ai member is not admitted: proxied MCP executions are recorded and scanned but not paired with a model intent"
        );
    }
    let engine = engine.with_mcp_audit(crate::services::proxy::service::McpAuditSinks {
        executions: repo,
        intent_claims,
        ingest: Arc::clone(state.get::<Arc<systemprompt_mcp::ArtifactIngest>>()?),
    });

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
