//! Axum handler adapters for the MCP and agent proxy routes, each delegating
//! to [`ProxyEngine::proxy_request`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_runtime::AppContext;

use super::{ProxyEngine, ProxyKind, ProxyTarget};

impl ProxyEngine {
    pub async fn handle_mcp_request_with_path(
        &self,
        path_params: Path<(String, String)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Response<Body> {
        let Path((service_name, path)) = path_params;
        let target = ProxyTarget {
            service_name: &service_name,
            path: &path,
            kind: ProxyKind::Mcp,
        };
        match self.proxy_request(target, request, ctx).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        }
    }

    pub async fn handle_agent_request(
        &self,
        path_params: Path<(String,)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Result<Response<Body>, StatusCode> {
        let Path((service_name,)) = path_params;
        let target = ProxyTarget {
            service_name: &service_name,
            path: "",
            kind: ProxyKind::Agent,
        };
        self.proxy_request(target, request, ctx)
            .await
            .map_err(|e| e.to_status_code())
    }

    pub async fn handle_agent_request_with_path(
        &self,
        path_params: Path<(String, String)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Result<Response<Body>, StatusCode> {
        let Path((service_name, path)) = path_params;
        let target = ProxyTarget {
            service_name: &service_name,
            path: &path,
            kind: ProxyKind::Agent,
        };
        self.proxy_request(target, request, ctx)
            .await
            .map_err(|e| e.to_status_code())
    }
}
