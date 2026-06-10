//! Reverse-proxy engine for MCP and agent backends.
//!
//! [`ProxyEngine`] resolves a service by name, enforces access via the proxy
//! auth boundary, forwards the request to the local backend port, and streams
//! the response back (with SSE keep-alive). For MCP it also maintains the
//! session-identity cache so a session-only follow-up request can be enriched
//! with the identity established on the authenticated initialize call.

mod handlers;
mod mcp_session;

use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::response::Response;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_database::ServiceConfig;
use systemprompt_identifiers::AgentName;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tokio::sync::RwLock;

use super::auth::{AccessValidator, build_mcp_unknown_service_challenge};
use super::backend::{HeaderInjector, ProxyError, RequestBuilder, ResponseHandler, UrlResolver};
use super::client::ClientPool;
use super::resolver::ServiceResolver;
use mcp_session::SessionCache;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    Mcp,
    Agent,
}

#[derive(Debug)]
pub struct ProxyTarget<'a> {
    pub service_name: &'a str,
    pub path: &'a str,
    pub kind: ProxyKind,
}

#[derive(Debug, Clone)]
pub struct ProxyEngine {
    client_pool: ClientPool,
    session_cache: SessionCache,
}

impl Default for ProxyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyEngine {
    pub fn new() -> Self {
        Self {
            client_pool: ClientPool::new(),
            session_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn proxy_request(
        &self,
        target: ProxyTarget<'_>,
        request: Request<Body>,
        ctx: AppContext,
    ) -> Result<Response<Body>, ProxyError> {
        let ProxyTarget {
            service_name,
            path,
            kind: proxy_kind,
        } = target;
        if request.extensions().get::<RequestContext>().is_none() {
            tracing::warn!("RequestContext missing from request extensions");
        }

        let service = match ServiceResolver::resolve(service_name, &ctx).await {
            Ok(svc) => svc,
            Err(err) => {
                return Err(unknown_service_error(
                    service_name,
                    proxy_kind,
                    &request,
                    &ctx,
                    err,
                ));
            },
        };

        let req_ctx = request.extensions().get::<RequestContext>().cloned();
        let authenticated_user = AccessValidator::validate(
            request.headers(),
            service_name,
            &service,
            &ctx,
            req_ctx.as_ref(),
        )
        .await?;

        let backend_url = UrlResolver::build_backend_url("http", "127.0.0.1", service.port, path);

        let method_str = request.method().to_string();
        let request_headers = request.headers().clone();
        let mut headers = request_headers.clone();
        let query = request.uri().query();
        let full_url = UrlResolver::append_query_params(backend_url, query);

        let req_context = self
            .build_forward_context(req_ctx, &service, service_name, &request_headers)
            .await?;

        inject_forward_headers(&mut headers, &req_context, service_name);

        let response = self
            .send_to_backend(request, &full_url, &headers, service_name)
            .await?;

        if service.module_name == "mcp" {
            mcp_session::handle_mcp_response(mcp_session::McpResponseCtx {
                cache: &self.session_cache,
                response: &response,
                request_headers: &request_headers,
                req_context: &req_context,
                authenticated_user: authenticated_user.as_ref(),
                service_name,
                method_str: &method_str,
            })
            .await;
        }

        match ResponseHandler::build_response(response) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                tracing::error!(service = %service_name, error = %e, "Failed to build response");
                Err(ProxyError::InvalidResponse {
                    service: service_name.to_owned(),
                    reason: format!("Failed to build response: {e}"),
                })
            },
        }
    }

    async fn build_forward_context(
        &self,
        req_ctx: Option<RequestContext>,
        service: &ServiceConfig,
        service_name: &str,
        request_headers: &HeaderMap,
    ) -> Result<RequestContext, ProxyError> {
        let mut req_context = req_ctx.ok_or_else(|| ProxyError::MissingContext {
            message: "Request context required - proxy cannot operate without authentication"
                .to_owned(),
        })?;

        if service.module_name == "agent" || service.module_name == "mcp" {
            req_context = req_context.with_agent_name(AgentName::new(service_name.to_owned()));
        }

        if service.module_name == "mcp" && req_context.auth_token().as_str().is_empty() {
            req_context = mcp_session::enrich_with_cached_identity(
                &self.session_cache,
                request_headers,
                req_context,
                service_name,
            )
            .await;
        }

        Ok(req_context)
    }

    async fn send_to_backend(
        &self,
        request: Request<Body>,
        full_url: &str,
        headers: &HeaderMap,
        service_name: &str,
    ) -> Result<reqwest::Response, ProxyError> {
        let method_str = request.method().to_string();

        let body = RequestBuilder::extract_body(request.into_body())
            .await
            .map_err(|e| ProxyError::BodyExtractionFailed { source: e })?;

        let reqwest_method = RequestBuilder::parse_method(&method_str)
            .map_err(|reason| ProxyError::InvalidMethod { reason })?;

        let client = self.client_pool.get_default_client();

        let req_builder =
            RequestBuilder::build_request(&client, reqwest_method, full_url, headers, body);

        req_builder.send().await.map_err(|e| {
            tracing::error!(service = %service_name, url = %full_url, error = %e, "Connection failed");
            ProxyError::ConnectionFailed {
                service: service_name.to_owned(),
                url: full_url.to_owned(),
                source: e,
            }
        })
    }
}

fn unknown_service_error(
    service_name: &str,
    proxy_kind: ProxyKind,
    request: &Request<Body>,
    ctx: &AppContext,
    err: ProxyError,
) -> ProxyError {
    if proxy_kind == ProxyKind::Mcp && matches!(err, ProxyError::ServiceNotFound { .. }) {
        let req_ctx = request.extensions().get::<RequestContext>().cloned();
        if let Some(challenge) = build_mcp_unknown_service_challenge(
            service_name,
            request.headers(),
            ctx,
            req_ctx.as_ref(),
        ) {
            return challenge;
        }
    }
    err
}

fn inject_forward_headers(
    headers: &mut HeaderMap,
    req_context: &RequestContext,
    service_name: &str,
) {
    let has_auth_before = headers.get("authorization").is_some();
    let ctx_has_token = !req_context.auth_token().as_str().is_empty();

    HeaderInjector::inject_context(headers, req_context);

    let has_auth_after = headers.get("authorization").is_some();
    tracing::debug!(
        service = %service_name,
        has_auth_before = has_auth_before,
        ctx_has_token = ctx_has_token,
        has_auth_after = has_auth_after,
        "Proxy forwarding request"
    );
}
