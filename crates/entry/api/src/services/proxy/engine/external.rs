//! MCP-over-HTTP forwarding for external servers.
//!
//! An external MCP server has no local backend port; instead the gateway mints
//! a per-user provider bearer server-side and forwards the MCP frames to the
//! provider endpoint, withholding the systemprompt credential and the provider
//! URL from the client. A client-mediated `tools/call` is audited under the
//! calling user via the response tap.

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::Response;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::services::client::McpClient;
use systemprompt_mcp::{McpDomainError, McpServerConfig};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use super::super::audit::{self, McpAudit, parse_tool_call};
use super::super::auth::{AccessValidator, mcp_oauth_requirement};
use super::super::backend::{ProxyError, RequestBuilder, ResponseHandler};
use super::ProxyEngine;

const MCP_PASSTHROUGH_HEADERS: [&str; 4] = [
    "content-type",
    "accept",
    "mcp-session-id",
    "mcp-protocol-version",
];

impl ProxyEngine {
    pub(super) async fn proxy_external_mcp(
        &self,
        service_name: &str,
        request: Request<Body>,
        ctx: AppContext,
        server_config: McpServerConfig,
    ) -> Result<Response<Body>, ProxyError> {
        let req_ctx = request
            .extensions()
            .get::<RequestContext>()
            .cloned()
            .ok_or_else(|| ProxyError::MissingContext {
                message: "external MCP proxy requires an authenticated request context".to_owned(),
            })?;

        let requirement = mcp_oauth_requirement(&ctx, service_name).await?;
        AccessValidator::validate_with_requirement(
            request.headers(),
            service_name,
            &requirement,
            &ctx,
            Some(&req_ctx),
        )?;

        let target = McpClient::resolve_external_proxy_target(&server_config, &req_ctx)
            .await
            .map_err(|e| map_resolve_error(service_name, e))?;

        let method_str = request.method().to_string();
        let incoming_headers = request.headers().clone();
        let body = RequestBuilder::extract_body(request.into_body())
            .await
            .map_err(|source| ProxyError::BodyExtractionFailed { source })?;

        let audit = build_audit(&ctx, &req_ctx, service_name, &body);
        let outbound = outbound_headers(&incoming_headers, target.headers);

        let method = RequestBuilder::parse_method(&method_str)
            .map_err(|reason| ProxyError::InvalidMethod { reason })?;
        let client = self.client_pool.get_default_client();
        let response = RequestBuilder::build_request(&client, method, &target.url, &outbound, body)
            .send()
            .await
            .map_err(|source| ProxyError::ConnectionFailed {
                service: service_name.to_owned(),
                url: target.url.clone(),
                source,
            })?;

        let to_invalid = |reason| ProxyError::InvalidResponse {
            service: service_name.to_owned(),
            reason,
        };
        match audit {
            Some(audit) => audit::record(response, audit).await.map_err(to_invalid),
            None => ResponseHandler::build_response(response).map_err(to_invalid),
        }
    }
}

fn outbound_headers(incoming: &HeaderMap, provider: HashMap<HeaderName, HeaderValue>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for name in MCP_PASSTHROUGH_HEADERS {
        if let Some(value) = incoming.get(name) {
            headers.insert(HeaderName::from_static(name), value.clone());
        }
    }
    for (name, value) in provider {
        headers.insert(name, value);
    }
    headers
}

fn build_audit(
    ctx: &AppContext,
    req_ctx: &RequestContext,
    service_name: &str,
    body: &[u8],
) -> Option<McpAudit> {
    let invocation = parse_tool_call(body)?;
    match ToolUsageRepository::new(ctx.db_pool()) {
        Ok(repo) => Some(McpAudit::new(
            std::sync::Arc::new(repo),
            req_ctx.clone(),
            service_name.to_owned(),
            invocation,
        )),
        Err(e) => {
            tracing::warn!(service = %service_name, error = %e, "Tool-usage repository unavailable; external MCP call not audited");
            None
        },
    }
}

fn map_resolve_error(service_name: &str, error: McpDomainError) -> ProxyError {
    match error {
        McpDomainError::AuthRequired(_) => ProxyError::AuthenticationRequired {
            service: service_name.to_owned(),
        },
        McpDomainError::ExternalAuthUnavailable { message, .. } => ProxyError::ServiceNotRunning {
            service: service_name.to_owned(),
            status: message,
        },
        other => ProxyError::InvalidResponse {
            service: service_name.to_owned(),
            reason: other.to_string(),
        },
    }
}
