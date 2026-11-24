//! MCP-over-HTTP forwarding for external servers.
//!
//! An external MCP server has no local backend port. The engine authorises
//! the caller against the server's OAuth requirement and the context it
//! names, then opens the caller's [`ExternalUpstream`] through the MCP
//! backends: the provider endpoint and the per-user provider headers, minted
//! server-side so the systemprompt credential and the provider URL never
//! reach the client. The engine bounds and reads the body, lets the upstream
//! fix the configured arguments, governs a `tools/call` (see
//! [`super::external_governance`]), forwards the frames and taps the
//! response with the call's audit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::ServiceName;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::McpServerConfig;
use systemprompt_runtime::AppContext;

use super::super::auth::{AccessValidator, mcp_oauth_requirement};
use super::super::backend::{MCP_REQUEST_BODY_LIMIT, ProxyError, RequestBuilder, ResponseHandler};
use super::ProxyService;
use super::external_governance::KernelCallGovernor;
use super::outbound::MCP_PASSTHROUGH_HEADERS;

// Why: takes the headers and the context rather than the request — a
// `Body` is not `Sync`, so holding `&Request` across the await would make
// the proxy future `!Send` and the router refuse it.
async fn authorised_context(
    ctx: &AppContext,
    service_name: &ServiceName,
    headers: &HeaderMap,
    req_ctx: Option<RequestContext>,
) -> Result<RequestContext, ProxyError> {
    let req_ctx = req_ctx.ok_or_else(|| ProxyError::MissingContext {
        message: "external MCP proxy requires an authenticated request context".to_owned(),
    })?;
    let requirement = mcp_oauth_requirement(ctx, service_name).await?;
    let user = AccessValidator::validate_with_requirement(
        headers,
        service_name,
        &requirement,
        ctx,
        Some(&req_ctx),
    )?;
    super::context_owner::ensure_owned(ctx, service_name, headers, user.as_ref(), Some(&req_ctx))
        .await?;
    Ok(req_ctx)
}

impl ProxyService {
    pub(super) async fn proxy_external_mcp(
        &self,
        service_name: &ServiceName,
        request: Request<Body>,
        ctx: AppContext,
        server_config: McpServerConfig,
    ) -> Result<Response<Body>, ProxyError> {
        let backend_error = |error| ProxyError::from_backend(service_name, error);
        let req_ctx = authorised_context(
            &ctx,
            service_name,
            request.headers(),
            request.extensions().get::<RequestContext>().cloned(),
        )
        .await?;

        let upstream = self
            .mcp_backends(service_name)?
            .open_external(&server_config, &req_ctx)
            .await
            .map_err(backend_error)?;

        let method_str = request.method().to_string();
        let incoming_headers = request.headers().clone();
        if let Some(session) =
            incoming_headers.get(systemprompt_identifiers::headers::MCP_SESSION_ID)
            && !upstream.accepts(session).await.map_err(backend_error)?
        {
            return Ok((
                StatusCode::NOT_FOUND,
                "MCP session expired; initialize again",
            )
                .into_response());
        }
        RequestBuilder::ensure_declared_length(&incoming_headers, MCP_REQUEST_BODY_LIMIT)?;
        let mut body =
            RequestBuilder::read_body(request.into_body(), MCP_REQUEST_BODY_LIMIT).await?;
        let audit = upstream
            .prepare(&mut body, &KernelCallGovernor::new(&ctx))
            .await
            .map_err(backend_error)?;
        let outbound = outbound_headers(&incoming_headers, upstream.headers());

        let method = RequestBuilder::parse_method(&method_str)?;
        let client = self.client_pool.get_default_client();
        let response = RequestBuilder::build_request(
            &client,
            method,
            upstream.url(),
            &outbound,
            (!body.is_empty()).then(|| reqwest::Body::from(body)),
        )
        .send()
        .await
        .map_err(|source| ProxyError::ConnectionFailed {
            service: service_name.to_string(),
            url: upstream.url().to_owned(),
            source,
        })?;

        if method_str == "DELETE" {
            if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
                upstream.forget(&incoming_headers).await.map_err(backend_error)?;
            }
        } else if response.status().is_success() {
            upstream
                .remember(response.headers())
                .await
                .map_err(backend_error)?;
        }
        ResponseHandler::build_audited_response(response, audit)
            .await
            .map_err(|source| ProxyError::InvalidResponse {
                service: service_name.to_string(),
                source,
            })
    }
}

pub fn outbound_headers(incoming: &HeaderMap, provider: &HeaderMap) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for name in MCP_PASSTHROUGH_HEADERS {
        if let Some(value) = incoming.get(name) {
            headers.insert(HeaderName::from_static(name), value.clone());
        }
    }
    for (name, value) in provider {
        headers.insert(name.clone(), value.clone());
    }
    headers
}
