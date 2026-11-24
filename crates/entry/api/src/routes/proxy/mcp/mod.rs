//! MCP reverse-proxy routes.
//!
//! Requests are forwarded to managed and external MCP servers through the
//! MCP backends the `mcp-http` member provides. Per-service discovery metadata lives in the `discovery` submodule; the
//! tool-execution lookup is the `mcp-http` member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;

use crate::kernel::ProxyService;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{any, get};
use systemprompt_extension::router::DynProxyBackendResolver;
use systemprompt_runtime::AppContext;

#[derive(Clone, Debug)]
pub struct McpState {
    pub ctx: AppContext,
}

pub fn router(ctx: &AppContext, backends: DynProxyBackendResolver) -> Router {
    let engine = ProxyService::new().with_mcp_backends(backends);
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
