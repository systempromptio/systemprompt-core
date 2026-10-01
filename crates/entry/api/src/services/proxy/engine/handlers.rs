//! Axum handler adapters for the MCP and agent proxy routes, each delegating
//! to [`ProxyEngine::proxy_request`]. The service-name path segment is parsed
//! once here; a malformed one answers 400 `invalid_identifier`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::ServiceName;
use systemprompt_runtime::AppContext;

use super::{ProxyEngine, ProxyKind, ProxyTarget};
use crate::services::proxy::backend::ProxyError;

impl ProxyEngine {
    pub async fn handle_mcp_request_with_path(
        &self,
        path_params: Path<(String, String)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Response<Body> {
        let Path((raw_service_name, path)) = path_params;
        let service_name = match ServiceName::try_new(raw_service_name) {
            Ok(name) => name,
            Err(error) => return ProxyError::from(error).into_response(),
        };
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
    ) -> Response<Body> {
        let Path((raw_service_name,)) = path_params;
        self.agent_request(&raw_service_name, "", ctx, request)
            .await
    }

    pub async fn handle_agent_request_with_path(
        &self,
        path_params: Path<(String, String)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Response<Body> {
        let Path((raw_service_name, path)) = path_params;
        self.agent_request(&raw_service_name, &path, ctx, request)
            .await
    }

    async fn agent_request(
        &self,
        raw_service_name: &str,
        path: &str,
        ctx: AppContext,
        request: Request<Body>,
    ) -> Response<Body> {
        let service_name = match ServiceName::try_new(raw_service_name) {
            Ok(name) => name,
            Err(error) => return ProxyError::from(error).into_response(),
        };
        let target = ProxyTarget {
            service_name: &service_name,
            path,
            kind: ProxyKind::Agent,
        };
        match self.proxy_request(target, request, ctx).await {
            Ok(response) => response,
            Err(e) => e.to_status_code().into_response(),
        }
    }
}
