//! The seam that sends one prepared HTTP request, and its reqwest
//! implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, TryStreamExt};
use url::Url;

use crate::llm::error::TransportError;

/// The body of a response, as chunks of bytes in arrival order.
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, TransportError>> + Send>>;

/// A prepared `POST`: URL, headers (credentials included) and the JSON body.
#[derive(Clone)]
pub struct HttpRequest {
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("HttpRequest")
            .field("url", &self.url.as_str())
            .field("headers", &names)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// The response head and a stream of body bytes.
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: ByteStream,
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("headers", &names)
            .finish_non_exhaustive()
    }
}

/// Sends one prepared request and resolves once the response head has arrived.
///
/// The stream entry point takes `&dyn Transport` so the caller can inject a
/// signing or test transport; `#[async_trait]` keeps the trait object-safe for
/// that `dyn` use.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// Connection timeouts. There is no total timeout: a stream may run as long as
/// data keeps arriving, and `idle` bounds the wait for each next chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    pub connect: Duration,
    pub idle: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(15),
            idle: Duration::from_secs(300),
        }
    }
}

/// The production transport over reqwest (rustls).
#[derive(Debug, Clone)]
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, TransportError> {
        Self::with_timeouts(Timeouts::default())
    }

    pub fn with_timeouts(timeouts: Timeouts) -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .connect_timeout(timeouts.connect)
            .read_timeout(timeouts.idle)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .map_err(TransportError::Client)?;
        Ok(Self { client })
    }

    pub const fn from_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

fn classify(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else if error.is_connect() {
        TransportError::Connect(error)
    } else {
        TransportError::Http(error)
    }
}

#[async_trait::async_trait]
impl Transport for HttpTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let HttpRequest { url, headers, body } = request;
        let mut builder = self.client.post(url).body(body);
        for (name, value) in &headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let response = builder.send().await.map_err(classify)?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|text| (name.as_str().to_owned(), text.to_owned()))
            })
            .collect();
        let body: ByteStream = Box::pin(response.bytes_stream().map_err(classify));
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}
