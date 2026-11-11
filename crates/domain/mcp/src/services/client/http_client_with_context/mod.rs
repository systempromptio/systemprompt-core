//! Context-propagating HTTP client for the MCP streamable-HTTP transport.
//!
//! [`HttpClientWithContext`] implements rmcp's `StreamableHttpClient`,
//! injecting the active [`RequestContext`] headers and bearer token onto every
//! GET/POST/ DELETE so authentication and trace context flow through to
//! downstream MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod limits;
pub mod metadata;
mod transport;

pub use limits::McpClientLimits;

use crate::services::client::challenge::{AuthChallenge, McpTransportError};
use http::header::AUTHORIZATION;
use http::{HeaderName, HeaderValue};
use rmcp::model::ClientCapabilities;
use std::collections::HashMap;
use systemprompt_client::{GuardedClient, GuardedClientConfig, guarded_client_builder};
use systemprompt_models::RequestContext;
use systemprompt_models::net::{HTTP_AUTH_VERIFY_TIMEOUT, HTTP_KEEPALIVE, HTTP_POOL_IDLE_TIMEOUT};
use systemprompt_models::oauth::ProtectedResourceMetadata;
use systemprompt_traits::ContextPropagation;

#[derive(Clone, Debug)]
pub struct HttpClientWithContext {
    client: reqwest::Client,
    metadata_client: GuardedClient,
    context: RequestContext,
    forward_context: bool,
    outbound_headers: HashMap<HeaderName, HeaderValue>,
    client_capabilities: ClientCapabilities,
    limits: McpClientLimits,
}

impl HttpClientWithContext {
    pub fn new(
        context: RequestContext,
        limits: McpClientLimits,
    ) -> Result<Self, McpTransportError> {
        Self::build(context, true, HashMap::new(), limits)
    }

    pub(super) const fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub(super) const fn limits(&self) -> &McpClientLimits {
        &self.limits
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
        let request = match self.metadata_client.get(metadata_url) {
            Ok(request) => request,
            Err(e) => {
                tracing::debug!(
                    metadata_url,
                    error = %e,
                    "MCP server advertised an unreachable resource_metadata URL"
                );
                return None;
            },
        };

        let response = match request.send().await {
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

        match limits::read_capped_json::<ProtectedResourceMetadata>(
            response,
            limits::MAX_RESOURCE_METADATA_BYTES,
        )
        .await
        {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                tracing::debug!(
                    metadata_url,
                    error = %e,
                    "MCP resource metadata is oversized or not valid RFC 9728 JSON"
                );
                None
            },
        }
    }

    pub fn external(
        context: RequestContext,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
        limits: McpClientLimits,
    ) -> Result<Self, McpTransportError> {
        Self::build(context, false, outbound_headers, limits)
    }

    pub fn forwarding(
        context: RequestContext,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
        limits: McpClientLimits,
    ) -> Result<Self, McpTransportError> {
        Self::build(context, true, outbound_headers, limits)
    }

    #[must_use]
    pub fn with_client_capabilities(mut self, capabilities: ClientCapabilities) -> Self {
        self.client_capabilities = capabilities;
        self
    }

    fn build(
        context: RequestContext,
        forward_context: bool,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
        limits: McpClientLimits,
    ) -> Result<Self, McpTransportError> {
        let config = GuardedClientConfig::for_configured()
            .without_timeout()
            .with_connect_timeout(limits.connect_timeout());
        let metadata_client = GuardedClient::untrusted(
            &GuardedClientConfig::for_untrusted().with_timeout(HTTP_AUTH_VERIFY_TIMEOUT),
        )?;
        let client = guarded_client_builder(&config)
            .tcp_keepalive(Some(HTTP_KEEPALIVE))
            .pool_idle_timeout(HTTP_POOL_IDLE_TIMEOUT)
            .build()?;

        Ok(Self {
            client,
            metadata_client,
            context,
            forward_context,
            outbound_headers,
            client_capabilities: super::capabilities::client_capabilities(false),
            limits,
        })
    }

    fn add_context_headers(
        &self,
        builder: reqwest::RequestBuilder,
        auth_token: Option<String>,
    ) -> reqwest::RequestBuilder {
        let mut builder = builder;

        if self.forward_context {
            for (key, value) in &self.context.to_headers() {
                if *key != AUTHORIZATION {
                    builder = builder.header(key, value);
                }
            }
        }

        for (key, value) in &self.outbound_headers {
            builder = builder.header(key, value);
        }

        match (auth_token, self.context.auth_token()) {
            (Some(token), _) => builder.bearer_auth(token),
            (None, Some(context_token)) if self.forward_context => {
                builder.bearer_auth(context_token.as_str())
            },
            (None, _) => builder,
        }
    }
}
