//! The outbound leg of a managed proxy request: the forwarded request context
//! and the bounded, streamed send to the backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::http::header::CONTENT_LENGTH;
use systemprompt_database::{ServiceConfig, ServiceModule};
use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_models::RequestContext;

use super::super::backend::{ProxyError, RequestBuilder, is_length_limit};
use super::ProxyService;

#[derive(Debug, Clone, Copy)]
pub(super) struct Upstream<'a> {
    pub(super) url: &'a str,
    pub(super) headers: &'a HeaderMap,
    pub(super) service_name: &'a ServiceName,
    pub(super) body_limit: usize,
}

impl ProxyService {
    pub(super) async fn send_to_backend(
        &self,
        request: Request<Body>,
        upstream: &Upstream<'_>,
    ) -> Result<reqwest::Response, ProxyError> {
        let Upstream {
            url: full_url,
            headers,
            service_name,
            body_limit,
        } = *upstream;
        let method = RequestBuilder::parse_method(request.method().as_str())?;
        let has_body =
            RequestBuilder::carries_body(&method) || request.headers().contains_key(CONTENT_LENGTH);
        let body = has_body.then(|| RequestBuilder::streamed_body(request.into_body(), body_limit));

        let client = self.client_pool.get_default_client();
        RequestBuilder::build_request(&client, method, full_url, headers, body)
            .send()
            .await
            .map_err(|source| {
                if is_length_limit(&source) {
                    ProxyError::PayloadTooLarge { limit: body_limit }
                } else {
                    ProxyError::ConnectionFailed {
                        service: service_name.to_string(),
                        url: full_url.to_owned(),
                        source,
                    }
                }
            })
    }
}

pub(super) fn forward_context(
    req_ctx: Option<RequestContext>,
    service: &ServiceConfig,
    service_name: &ServiceName,
) -> Result<RequestContext, ProxyError> {
    let req_context = req_ctx.ok_or_else(|| ProxyError::MissingContext {
        message: "Request context required - proxy cannot operate without authentication"
            .to_owned(),
    })?;
    if service.module_name != ServiceModule::Agent {
        return Ok(req_context);
    }
    let agent_name = AgentName::try_new(service_name.as_str()).map_err(|source| {
        ProxyError::InvalidServiceName {
            service: service_name.to_string(),
            source,
        }
    })?;
    Ok(req_context.with_agent_name(agent_name))
}
