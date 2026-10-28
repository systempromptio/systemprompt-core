//! Reverse-proxy service for MCP and agent backends.
//!
//! [`ProxyService`] resolves a service by name, enforces access via the proxy
//! auth boundary, forwards the request to the local backend port, and streams
//! the response back (with SSE keep-alive). For MCP it also maintains the
//! persisted session identity so a session-only follow-up request can be
//! enriched, on any replica, with the identity established on the
//! authenticated initialize call. A node that supervises no processes, or a
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
pub mod mcp_session;
pub mod owner;
mod target;

use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_config::SecretsService;
use systemprompt_database::{ServiceConfig, ServiceModule};
use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_manifest::Config;
use systemprompt_mcp::repository::McpProxyIdentityRepository;
use systemprompt_mcp::services::client::McpClient;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_security::credential::TokenCache;

use super::auth::AccessValidator;
use super::backend::{HeaderInjector, ProxyError, RequestBuilder, ResponseHandler, UrlResolver};
use super::client::ClientPool;
use super::resolver::ServiceResolver;
use target::{external_server, unknown_service_error};

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

#[derive(Debug, Clone)]
pub struct ProxyService {
    client_pool: ClientPool,
    identities: Arc<McpProxyIdentityRepository>,
    intent_claims: Option<systemprompt_mcp::IntentClaimService>,
    artifact_ingest: Option<Arc<systemprompt_mcp::ArtifactIngest>>,
    mcp_client: Arc<McpClient>,
    secrets: Arc<SecretsService>,
    owners: Arc<owner::OwnerCache>,
}

impl ProxyService {
    pub fn new(
        identities: Arc<McpProxyIdentityRepository>,
        connector_tokens: Arc<TokenCache>,
        services: &systemprompt_manifest::services::ServicesView,
        config: Arc<Config>,
        secrets: Arc<SecretsService>,
    ) -> Self {
        Self {
            client_pool: ClientPool::new(),
            identities,
            intent_claims: None,
            artifact_ingest: None,
            mcp_client: Arc::new(McpClient::new(
                connector_tokens,
                systemprompt_mcp::services::DeploymentService::new(services.clone()),
                config,
                Arc::clone(&secrets),
            )),
            secrets,
            owners: Arc::new(owner::OwnerCache::default()),
        }
    }

    pub(super) fn at_rest(&self) -> Result<AtRestCipher, ProxyError> {
        Ok(AtRestCipher::from_secrets(&self.secrets.get())?)
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
            && !request.headers().contains_key(owner::FORWARDED_TO_OWNER)
            && let Some(owner) = self.process_owner(service_name, &ctx).await?
        {
            return self.forward_to(&owner, service_name, request).await;
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

        if service.module_name == ServiceModule::Mcp {
            let at_rest = self.at_rest()?;
            mcp_session::handle_mcp_response(mcp_session::McpResponseCtx {
                identities: &self.identities,
                at_rest: &at_rest,
                response: &response,
                request_headers: &request_headers,
                req_context: &req_context,
                authenticated_user: authenticated_user.as_ref(),
                service_name,
                method_str: &method_str,
            })
            .await;
        }

        ResponseHandler::build_response(response).map_err(|source| ProxyError::InvalidResponse {
            service: service_name.to_string(),
            source,
        })
    }

    async fn build_forward_context(
        &self,
        req_ctx: Option<RequestContext>,
        service: &ServiceConfig,
        service_name: &ServiceName,
        request_headers: &HeaderMap,
    ) -> Result<RequestContext, ProxyError> {
        let mut req_context = req_ctx.ok_or_else(|| ProxyError::MissingContext {
            message: "Request context required - proxy cannot operate without authentication"
                .to_owned(),
        })?;

        if service.module_name == ServiceModule::Agent {
            let agent_name = AgentName::try_new(service_name.as_str()).map_err(|source| {
                ProxyError::InvalidServiceName {
                    service: service_name.to_string(),
                    source,
                }
            })?;
            req_context = req_context.with_agent_name(agent_name);
        }

        if service.module_name == ServiceModule::Mcp && req_context.auth_token().is_none() {
            req_context = mcp_session::enrich_with_cached_identity(
                &self.identities,
                &self.at_rest()?,
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
        service_name: &ServiceName,
    ) -> Result<reqwest::Response, ProxyError> {
        let method_str = request.method().to_string();

        let body = RequestBuilder::extract_body(request.into_body())
            .await
            .map_err(|e| ProxyError::BodyExtractionFailed { source: e })?;

        let reqwest_method = RequestBuilder::parse_method(&method_str)?;

        let client = self.client_pool.get_default_client();

        let req_builder =
            RequestBuilder::build_request(&client, reqwest_method, full_url, headers, body);

        req_builder
            .send()
            .await
            .map_err(|e| ProxyError::ConnectionFailed {
                service: service_name.to_string(),
                url: full_url.to_owned(),
                source: e,
            })
    }
}

fn inject_forward_headers(
    headers: &mut HeaderMap,
    req_context: &RequestContext,
    service_name: &ServiceName,
) {
    let has_auth_before = headers.get("authorization").is_some();
    let ctx_has_token = req_context.auth_token().is_some();

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
