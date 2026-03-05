use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{AgentName, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_runtime::AppContext;
use tokio::sync::RwLock;

use super::auth::AccessValidator;
use super::backend::{HeaderInjector, ProxyError, RequestBuilder, ResponseHandler, UrlResolver};
use super::client::ClientPool;
use super::resolver::ServiceResolver;

#[derive(Clone, Debug)]
struct ProxySessionIdentity {
    user_id: String,
    user_type: String,
    permissions: Vec<Permission>,
    auth_token: String,
}

type SessionCache = Arc<RwLock<HashMap<String, ProxySessionIdentity>>>;

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
        service_name: &str,
        path: &str,
        request: Request<Body>,
        ctx: AppContext,
    ) -> Result<Response<Body>, ProxyError> {
        if request.extensions().get::<RequestContext>().is_none() {
            tracing::warn!("RequestContext missing from request extensions");
        }

        let service = ServiceResolver::resolve(service_name, &ctx).await?;

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

        let mut req_context = req_ctx.clone().ok_or_else(|| ProxyError::MissingContext {
            message: "Request context required - proxy cannot operate without authentication"
                .to_string(),
        })?;

        if service.module_name == "agent" || service.module_name == "mcp" {
            req_context = req_context.with_agent_name(AgentName::new(service_name.to_string()));
        }

        if service.module_name == "mcp" && req_context.auth_token().as_str().is_empty() {
            if let Some(session_id) = request_headers
                .get("mcp-session-id")
                .and_then(|v| v.to_str().ok())
            {
                if let Some(identity) = self.session_cache.read().await.get(session_id) {
                    tracing::info!(
                        service = %service_name,
                        session_id = %session_id,
                        user_id = %identity.user_id,
                        "Enriching session-only request with cached identity"
                    );
                    req_context = req_context
                        .with_user_id(UserId::from(identity.user_id.clone()))
                        .with_user_type(
                            identity
                                .user_type
                                .parse()
                                .unwrap_or(systemprompt_models::auth::UserType::Unknown),
                        )
                        .with_auth_token(identity.auth_token.clone())
                        .with_user(AuthenticatedUser::new(
                            identity.user_id.parse().unwrap_or_default(),
                            String::new(),
                            String::new(),
                            identity.permissions.clone(),
                        ));
                }
            }
        }

        let has_auth_before = headers.get("authorization").is_some();
        let ctx_has_token = !req_context.auth_token().as_str().is_empty();

        HeaderInjector::inject_context(&mut headers, &req_context);

        let has_auth_after = headers.get("authorization").is_some();
        tracing::debug!(
            service = %service_name,
            has_auth_before = has_auth_before,
            ctx_has_token = ctx_has_token,
            has_auth_after = has_auth_after,
            "Proxy forwarding request"
        );

        let body = RequestBuilder::extract_body(request.into_body())
            .await
            .map_err(|e| ProxyError::BodyExtractionFailed { source: e })?;

        let reqwest_method = RequestBuilder::parse_method(&method_str)
            .map_err(|reason| ProxyError::InvalidMethod { reason })?;

        let client = self.client_pool.get_default_client();

        let req_builder =
            RequestBuilder::build_request(&client, reqwest_method, &full_url, &headers, body);

        let req_builder = req_builder.map_err(|status| ProxyError::InvalidResponse {
            service: service_name.to_string(),
            reason: format!("Failed to build request: {status}"),
        })?;

        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(service = %service_name, url = %full_url, error = %e, "Connection failed");
                return Err(ProxyError::ConnectionFailed {
                    service: service_name.to_string(),
                    url: full_url.clone(),
                    source: e,
                });
            },
        };

        if service.module_name == "mcp" {
            if let Some(session_id) = response
                .headers()
                .get("mcp-session-id")
                .and_then(|v| v.to_str().ok())
            {
                if let Some(user) = &authenticated_user {
                    self.session_cache.write().await.insert(
                        session_id.to_string(),
                        ProxySessionIdentity {
                            user_id: user.id.to_string(),
                            user_type: req_context.user_type().to_string(),
                            permissions: user.permissions.clone(),
                            auth_token: req_context.auth_token().as_str().to_string(),
                        },
                    );
                    tracing::info!(
                        service = %service_name,
                        session_id = %session_id,
                        user_id = %user.id,
                        "Cached session identity for MCP session"
                    );
                }
            }

            if method_str == "DELETE" {
                if let Some(session_id) = request_headers
                    .get("mcp-session-id")
                    .and_then(|v| v.to_str().ok())
                {
                    self.session_cache.write().await.remove(session_id);
                    tracing::debug!(session_id = %session_id, "Evicted session identity on DELETE");
                }
            }
        }

        match ResponseHandler::build_response(response) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                tracing::error!(service = %service_name, error = %e, "Failed to build response");
                Err(ProxyError::InvalidResponse {
                    service: service_name.to_string(),
                    reason: format!("Failed to build response: {e}"),
                })
            },
        }
    }

    pub async fn handle_mcp_request(
        &self,
        path_params: Path<(String,)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Response<Body> {
        let Path((service_name,)) = path_params;
        match self.proxy_request(&service_name, "", request, ctx).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        }
    }

    pub async fn handle_mcp_request_with_path(
        &self,
        path_params: Path<(String, String)>,
        State(ctx): State<AppContext>,
        request: Request<Body>,
    ) -> Response<Body> {
        let Path((service_name, path)) = path_params;
        match self.proxy_request(&service_name, &path, request, ctx).await {
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
        self.proxy_request(&service_name, "", request, ctx)
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
        self.proxy_request(&service_name, &path, request, ctx)
            .await
            .map_err(|e| e.to_status_code())
    }
}
