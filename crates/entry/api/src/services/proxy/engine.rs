use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::AgentName;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use super::auth::AccessValidator;
use super::backend::{HeaderInjector, ProxyError, RequestBuilder, ResponseHandler, UrlResolver};
use super::client::ClientPool;
use super::resolver::ServiceResolver;

#[derive(Debug, Clone)]
pub struct ProxyEngine {
    client_pool: ClientPool,
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
        AccessValidator::validate(
            request.headers(),
            service_name,
            &service,
            &ctx,
            req_ctx.as_ref(),
        )
        .await?;

        let backend_url = UrlResolver::build_backend_url("http", "127.0.0.1", service.port, path);

        let method_str = request.method().to_string();
        let mut headers = request.headers().clone();
        let query = request.uri().query();
        let full_url = UrlResolver::append_query_params(backend_url, query);

        let mut req_context = req_ctx.clone().ok_or_else(|| ProxyError::MissingContext {
            message: "Request context required - proxy cannot operate without authentication"
                .to_string(),
        })?;

        if service.module_name == "agent" || service.module_name == "mcp" {
            req_context = req_context.with_agent_name(AgentName::new(service_name.to_string()));
        }

        HeaderInjector::inject_context(&mut headers, &req_context);

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
