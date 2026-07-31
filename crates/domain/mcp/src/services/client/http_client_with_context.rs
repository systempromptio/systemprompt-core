//! Context-propagating HTTP client for the MCP streamable-HTTP transport.
//!
//! [`HttpClientWithContext`] implements rmcp's `StreamableHttpClient`,
//! injecting the active [`RequestContext`] headers and bearer token onto every
//! GET/POST/ DELETE so authentication and trace context flow through to
//! downstream MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::bounded_sse::{DEFAULT_MAX_SSE_EVENT_SIZE, bounded_sse_stream};
use super::challenge::{AuthChallenge, McpTransportError};
use futures::stream::BoxStream;
use http::header::WWW_AUTHENTICATE;
use http::{HeaderName, HeaderValue};
use reqwest::header::ACCEPT;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::common::http_header::{
    EVENT_STREAM_MIME_TYPE, HEADER_LAST_EVENT_ID, HEADER_SESSION_ID, JSON_MIME_TYPE,
};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};
use sse_stream::{Error as SseError, Sse};
use std::collections::HashMap;
use std::sync::Arc;
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
}

impl HttpClientWithContext {
    pub fn new(context: RequestContext) -> Self {
        Self::build(context, true, HashMap::new())
    }

    /// Turn a 401's `WWW-Authenticate` challenge into a typed error, enriched
    /// with whatever its RFC 9728 metadata says about how to authorize.
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

    /// Read the RFC 9728 metadata a challenge points at.
    ///
    /// The URL comes from the peer, so it goes through the outbound guard
    /// before it is dialled. An unreadable document downgrades the challenge
    /// rather than replacing it — the 401 stands on its own — but each failure
    /// is logged, since a metadata endpoint that never answers is a
    /// misconfiguration the operator cannot otherwise see.
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

    /// Client for a third-party external MCP server: the systemprompt JWT and
    /// `x-systemprompt-*` context headers are withheld so nothing internal
    /// reaches the third party; only `outbound_headers` (the resolved per-user
    /// bearer plus any static configured headers) are sent.
    pub fn external(
        context: RequestContext,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Self {
        Self::build(context, false, outbound_headers)
    }

    /// Client that forwards the systemprompt context and credential (internal /
    /// managed servers) while also sending any static configured headers.
    pub fn forwarding(
        context: RequestContext,
        outbound_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Self {
        Self::build(context, true, outbound_headers)
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

impl StreamableHttpClient for HttpClientWithContext {
    type Error = McpTransportError;

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        self.get_stream_with_max_sse_event_size(
            uri,
            session_id,
            last_event_id,
            auth_token,
            custom_headers,
            DEFAULT_MAX_SSE_EVENT_SIZE,
        )
        .await
    }

    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        let mut request_builder = self
            .client
            .get(uri.as_ref())
            .header(ACCEPT, EVENT_STREAM_MIME_TYPE);

        if let Some(ref session_id) = session_id {
            request_builder = request_builder.header(HEADER_SESSION_ID, session_id.as_ref());
        }

        request_builder = self.add_context_headers(request_builder);

        for (key, value) in &custom_headers {
            request_builder = request_builder.header(key, value);
        }

        if let Some(last_event_id) = last_event_id {
            request_builder = request_builder.header(HEADER_LAST_EVENT_ID, last_event_id);
        }
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| StreamableHttpError::Client(e.into()))?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        let response = response
            .error_for_status()
            .map_err(|e| StreamableHttpError::Client(e.into()))?;
        match response.headers().get(reqwest::header::CONTENT_TYPE) {
            Some(ct) => {
                if !ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes()) {
                    return Err(StreamableHttpError::UnexpectedContentType(Some(
                        String::from_utf8_lossy(ct.as_bytes()).to_string(),
                    )));
                }
            },
            None => {
                return Err(StreamableHttpError::UnexpectedContentType(None));
            },
        }
        Ok(bounded_sse_stream(
            response.bytes_stream(),
            max_sse_event_size,
        ))
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let mut request_builder = self.client.delete(uri.as_ref());

        request_builder = self.add_context_headers(request_builder);

        for (key, value) in &custom_headers {
            request_builder = request_builder.header(key, value);
        }

        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }
        let response = request_builder
            .header(HEADER_SESSION_ID, session.as_ref())
            .send()
            .await
            .map_err(|e| StreamableHttpError::Client(e.into()))?;

        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        let _response = response
            .error_for_status()
            .map_err(|e| StreamableHttpError::Client(e.into()))?;
        Ok(())
    }

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.post_message_with_max_sse_event_size(
            uri,
            message,
            session_id,
            auth_token,
            custom_headers,
            DEFAULT_MAX_SSE_EVENT_SIZE,
        )
        .await
    }

    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let mut request = self
            .client
            .post(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "));

        request = self.add_context_headers(request);

        for (key, value) in &custom_headers {
            request = request.header(key, value);
        }

        if let Some(auth_header) = auth_token {
            request = request.bearer_auth(auth_header);
        }
        if let Some(ref session_id) = session_id {
            request = request.header(HEADER_SESSION_ID, session_id.as_ref());
        }
        let response = request
            .json(&message)
            .send()
            .await
            .map_err(|e| StreamableHttpError::Client(e.into()))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let Some(header) = response.headers().get(WWW_AUTHENTICATE)
        {
            let header = header.to_str().map_err(|_e| {
                StreamableHttpError::UnexpectedServerResponse(std::borrow::Cow::from(
                    "invalid www-authenticate header value",
                ))
            })?;
            return Err(StreamableHttpError::Client(
                self.authorization_error(header).await,
            ));
        }
        let response = response
            .error_for_status()
            .map_err(|e| StreamableHttpError::Client(e.into()))?;
        if response.status() == reqwest::StatusCode::ACCEPTED {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        let content_type = response.headers().get(reqwest::header::CONTENT_TYPE);
        let session_id = response.headers().get(HEADER_SESSION_ID);
        let session_id = session_id.and_then(|v| v.to_str().ok()).map(str::to_owned);
        match content_type {
            Some(ct) if ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes()) => {
                let event_stream = bounded_sse_stream(response.bytes_stream(), max_sse_event_size);
                Ok(StreamableHttpPostResponse::Sse(event_stream, session_id))
            },
            Some(ct) if ct.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes()) => {
                let message: ServerJsonRpcMessage = response
                    .json()
                    .await
                    .map_err(|e| StreamableHttpError::Client(e.into()))?;
                Ok(StreamableHttpPostResponse::Json(message, session_id))
            },
            _ => Err(StreamableHttpError::UnexpectedContentType(
                content_type.map(|ct| String::from_utf8_lossy(ct.as_bytes()).to_string()),
            )),
        }
    }
}
