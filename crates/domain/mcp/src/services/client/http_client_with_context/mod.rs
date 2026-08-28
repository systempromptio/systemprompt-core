//! Context-propagating HTTP client for the MCP streamable-HTTP transport.
//!
//! [`HttpClientWithContext`] implements rmcp's `StreamableHttpClient`,
//! injecting the active [`RequestContext`] headers and bearer token onto every
//! GET/POST/ DELETE so authentication and trace context flow through to
//! downstream MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod metadata;
mod transport;

use crate::services::client::challenge::{AuthChallenge, McpTransportError};
use http::{HeaderName, HeaderValue};
use rmcp::model::ClientCapabilities;
use std::collections::HashMap;
use systemprompt_models::RequestContext;
use systemprompt_models::net::{
    HTTP_KEEPALIVE, HTTP_POOL_IDLE_TIMEOUT, HTTP_STREAM_CONNECT_TIMEOUT, validate_outbound_url,
};
use systemprompt_models::oauth::ProtectedResourceMetadata;
use systemprompt_traits::ContextPropagation;

#[derive(Clone, Debug)]
pub struct HttpClientWithContext {
    client: reqwest::Client,
    context: RequestContext,
    forward_context: bool,
    outbound_headers: HashMap<HeaderName, HeaderValue>,
    // Why: restated in every request's `_meta` from 2026-07-28 on. See
    // `stamp_request_metadata` — the value must match what `initialize`
    // declared, so it is supplied by whoever built the `ClientInfo`.
    client_capabilities: ClientCapabilities,
}

impl HttpClientWithContext {
    pub fn new(context: RequestContext) -> Self {
        Self::build(context, true, HashMap::new())
    }

    async fn authorization_error(&self, header: &str) -> McpTransportError {
        let challenge = AuthChallenge::parse(header);
        let metadata_url = challenge.resource_metadata.clone();
        let metadata = match metadata_url.as_deref() {
            Some(url) => self.fetch_protected_resource(url).await,
            None => None,
        };

        McpTransportError::AuthorizationRequired {
            reason: challenge
                .error_description
                .or(challenge.error)
                .unwrap_or_else(|| "the server requires authorization".to_owned()),
            resource: metadata.as_ref().map(|m| m.resource.clone()),
            metadata_url,
            authorization_servers: metadata
                .as_ref()
                .map(|m| m.authorization_servers.clone())
                .unwrap_or_default(),
            enterprise_managed: metadata
                .as_ref()
                .is_some_and(ProtectedResourceMetadata::requires_enterprise_managed_auth),
        }
    }

    async fn fetch_protected_resource(
        &self,
        metadata_url: &str,
    ) -> Option<ProtectedResourceMetadata> {
        let url = match validate_outbound_url(metadata_url) {
            Ok(url) => url,
            Err(e) => {
                tracing::debug!(
                    metadata_url,
                    error = %e,
                    "MCP server advertised an unreachable resource_metadata URL"
                );
                return None;
            },
        };

        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(e) => {
                tracing::debug!(
                    metadata_url,
                    error = %e,
                    "failed to fetch MCP resource metadata"
                );
                return None;
            },
        };
        if let Err(e) = response.error_for_status_ref() {
            tracing::debug!(
                metadata_url,
                status = %response.status(),
                error = %e,
                "MCP resource metadata endpoint refused the request"
            );
            return None;
        }

        match response.json::<ProtectedResourceMetadata>().await {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                tracing::debug!(
                    metadata_url,
                    error = %e,
                    "MCP resource metadata is not valid RFC 9728 JSON"
                );
                None
            },
        }
    }

    pub fn external(
        context: RequestContext,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Self {
        Self::build(context, false, outbound_headers)
    }

    pub fn forwarding(
        context: RequestContext,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Self {
        Self::build(context, true, outbound_headers)
    }

    // Why: the capabilities restated in `_meta` must be the ones `initialize`
    // declared. A caller that knows them (an elicitation-capable tool call)
    // sets them with `with_client_capabilities`; the default matches what
    // `client_capabilities(false)` sends.
    #[must_use]
    pub fn with_client_capabilities(mut self, capabilities: ClientCapabilities) -> Self {
        self.client_capabilities = capabilities;
        self
    }

    fn build(
        context: RequestContext,
        forward_context: bool,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(HTTP_STREAM_CONNECT_TIMEOUT)
            .tcp_keepalive(Some(HTTP_KEEPALIVE))
            .pool_idle_timeout(HTTP_POOL_IDLE_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::default());

        Self {
            client,
            context,
            forward_context,
            outbound_headers,
            client_capabilities: super::capabilities::client_capabilities(false),
        }
    }

    fn add_context_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut builder = builder;

        if self.forward_context {
            for (key, value) in &self.context.to_headers() {
                builder = builder.header(key, value);
            }

            if !self.context.auth_token().as_str().is_empty() {
                let auth_header = format!("Bearer {}", self.context.auth_token().as_str());
                builder = builder.header("Authorization", &auth_header);
            }
        }

        for (key, value) in &self.outbound_headers {
            builder = builder.header(key, value);
        }

        builder
    }
}
