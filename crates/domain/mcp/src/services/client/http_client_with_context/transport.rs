//! rmcp `StreamableHttpClient` implementation for [`HttpClientWithContext`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::HttpClientWithContext;
use super::metadata::stamp_request_metadata;
use crate::services::client::bounded_sse::{DEFAULT_MAX_SSE_EVENT_SIZE, bounded_sse_stream};
use crate::services::client::challenge::McpTransportError;
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
        mut message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        stamp_request_metadata(&mut message, &custom_headers, &self.client_capabilities);

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
