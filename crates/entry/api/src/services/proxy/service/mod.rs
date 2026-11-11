//! Reverse-proxy service for MCP and agent backends.
//!
//! [`ProxyService`] resolves a service by name, enforces access via the proxy
//! auth boundary, forwards the request to the local backend port, and streams
//! the response back (with SSE keep-alive). Every request is authenticated on
//! its own credential; the proxy builds the backend's headers itself from an
//! allow-list (see [`outbound`]), signs a short-lived hop token carrying the
//! verified identity for every request to a managed MCP server,
//! and, for MCP, binds each session to the server and principal that opened
//! it (see [`mcp_session`]), so a session id never stands in for a
//! credential. A node that supervises no processes, or a
//! worker that does not hold the process's lease, forwards to the owning
//! replica instead (see [`owner`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod external;
mod external_governance;
mod external_sessions;
pub mod fixed_arguments;
mod handlers;
mod hop;
pub mod mcp_session;
pub mod outbound;
pub mod owner;
mod target;
mod upstream;

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_config::SecretsService;
use systemprompt_database::{ServiceConfig, ServiceModule};
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::Config;
use systemprompt_mcp::repository::McpProxySessionRepository;
use systemprompt_mcp::services::client::McpClient;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, TokenAuthority};
use systemprompt_runtime::AppContext;
use systemprompt_security::credential::TokenCache;

use super::auth::AccessValidator;
use super::backend::{
    MCP_REQUEST_BODY_LIMIT, ProxyError, RequestBuilder, ResponseHandler, UrlResolver,
};
use super::client::ClientPool;
use super::resolver::ServiceResolver;
use target::{external_server, unknown_service_error};
use upstream::{Upstream, forward_context};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    Mcp,
    Agent,
}

#[derive(Debug)]
pub struct ProxyTarget<'a> {
    pub service_name: &'a ServiceName,
    pub path: &'a str,
    pub kind: ProxyKind,
}

#[derive(Debug)]
struct ManagedRequest<'a> {
    service: &'a ServiceConfig,
    service_name: &'a ServiceName,
    path: &'a str,
    user: Option<AuthenticatedUser>,
    req_ctx: Option<RequestContext>,
    tokens: &'a dyn TokenAuthority,
}

#[derive(Debug, Clone)]
pub struct ProxyService {
    client_pool: ClientPool,
    sessions: Arc<McpProxySessionRepository>,
    intent_claims: Option<systemprompt_mcp::IntentClaimService>,
    artifact_ingest: Option<Arc<systemprompt_mcp::ArtifactIngest>>,
    mcp_client: Arc<McpClient>,
    owners: Arc<owner::OwnerCache>,
}

impl ProxyService {
    pub fn new(
        sessions: Arc<McpProxySessionRepository>,
        connector_tokens: Arc<TokenCache>,
        services: &systemprompt_manifest::services::ServicesView,
        config: Arc<Config>,
        secrets: Arc<SecretsService>,
    ) -> Self {
        Self {
            client_pool: ClientPool::new(),
            sessions,
            intent_claims: None,
            artifact_ingest: None,
            mcp_client: Arc::new(McpClient::new(
                connector_tokens,
                systemprompt_mcp::services::DeploymentService::new(services.clone()),
                config,
                secrets,
                systemprompt_mcp::services::client::McpClientLimits::from(
                    &services.config().ai.mcp.resilience,
                ),
            )),
            owners: Arc::new(owner::OwnerCache::default()),
        }
    }

    #[must_use]
    pub fn with_tool_usage_repo(
        mut self,
        repo: Arc<systemprompt_mcp::repository::ToolUsageRepository>,
        intents: systemprompt_traits::DynToolCallIntentClaims,
    ) -> Self {
        self.intent_claims = Some(systemprompt_mcp::IntentClaimService::new(intents, repo));
        self
    }

    #[must_use]
    pub fn with_artifact_ingest(mut self, ingest: Arc<systemprompt_mcp::ArtifactIngest>) -> Self {
        self.artifact_ingest = Some(ingest);
        self
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

        if let Some(server_config) = external_server(proxy_kind, service_name, &ctx)? {
            return self
                .proxy_external_mcp(service_name, request, ctx, server_config)
                .await;
        }

        if !ctx.config().role.supervises_processes() {
            return self.forward_to_owner(service_name, request, &ctx).await;
        }
        if ctx.config().role.shares_process_ownership()
            && !owner::forwarded_by_replica(request.headers(), service_name, &ctx)
            && let Some(owner) = self.process_owner(service_name, &ctx).await?
        {
            return self.forward_to(&owner, service_name, request, &ctx).await;
        }

        let service = ServiceResolver::resolve(service_name, &ctx)
            .await
            .map_err(|err| unknown_service_error(service_name, proxy_kind, &request, &ctx, err))?;

        let req_ctx = request.extensions().get::<RequestContext>().cloned();
        let authenticated_user = AccessValidator::validate(
            request.headers(),
            service_name,
            &service,
            &ctx,
            req_ctx.as_ref(),
        )
        .await?;

        let tokens = ctx.token_authority();
        self.proxy_managed(
            ManagedRequest {
                service: &service,
                service_name,
                path,
                user: authenticated_user,
                req_ctx,
                tokens: tokens.as_ref(),
            },
            request,
        )
        .await
    }

    async fn proxy_managed(
        &self,
        managed: ManagedRequest<'_>,
        request: Request<Body>,
    ) -> Result<Response<Body>, ProxyError> {
        let ManagedRequest {
            service,
            service_name,
            path,
            user,
            req_ctx,
            tokens,
        } = managed;
        let principal = user.as_ref().map(|user| user.id.clone());
        let is_mcp = service.module_name == ServiceModule::Mcp;
        let body_limit = if is_mcp {
            MCP_REQUEST_BODY_LIMIT
        } else {
            systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES
        };
        RequestBuilder::ensure_declared_length(request.headers(), body_limit)?;
        if is_mcp {
            mcp_session::ensure_bound(
                &self.sessions,
                request.headers(),
                service_name,
                principal.as_ref(),
            )
            .await?;
        }

        let backend_url = UrlResolver::build_backend_url("http", "127.0.0.1", service.port, path);
        let full_url = UrlResolver::append_query_params(backend_url, request.uri().query());
        let method_str = request.method().to_string();
        let request_headers = request.headers().clone();

        let req_context = forward_context(req_ctx, service, service_name)?;
        let headers = if is_mcp {
            hop::managed_headers(
                tokens,
                &hop::HopGrant {
                    service_name,
                    context: &req_context,
                    user: user.as_ref(),
                    request_headers: &request_headers,
                },
            )?
        } else {
            outbound::agent_headers(&request_headers, &req_context)
        };
        tracing::debug!(service = %service_name, has_auth = headers.contains_key(AUTHORIZATION), "Proxy forwarding request");

        let response = self
            .send_to_backend(
                request,
                &Upstream {
                    url: &full_url,
                    headers: &headers,
                    service_name,
                    body_limit,
                },
            )
            .await?;

        if is_mcp {
            mcp_session::bind_issued_session(mcp_session::IssuedSession {
                sessions: &self.sessions,
                response: &response,
                request_headers: &request_headers,
                service_name,
                principal: principal.as_ref(),
                method_str: &method_str,
            })
            .await?;
        }

        ResponseHandler::build_response(response).map_err(|source| ProxyError::InvalidResponse {
            service: service_name.to_string(),
            source,
        })
    }
}
