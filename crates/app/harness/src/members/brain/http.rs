//! The request layer under [`crate::members::brain::BrainClient`]: one bounded JSON exchange
//! with a bearer.
//!
//! The URL is always resolved on the account's own origin
//! (`sp_protocol::origin`), redirects are never followed (the shared
//! [`systemprompt_device::client::HttpClient`] refuses them), every answer is read up to its
//! route's limit and refused past it, and a transport error is reported
//! without its URL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use bytes::Bytes;
use reqwest::Method;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap};
use systemprompt_device::client::{ClientError, TransportFailure};
use sp_protocol::origin::resolve_instance_path;
use sp_protocol::wire::envelope::ErrorEnvelope;
use sp_protocol::wire::headers;
use sp_protocol::{HOST_ID, Jwt};
use url::Url;

use crate::members::brain::error::{plain_key, protocol, refusal};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const SMALL_BODY: usize = 1024 * 1024;

#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) method: Method,
    pub(crate) url: Url,
    pub(crate) endpoint: &'static str,
    pub(crate) body: Option<Bytes>,
    pub(crate) limit: usize,
}

impl Call {
    pub(crate) const fn new(method: Method, url: Url, endpoint: &'static str) -> Self {
        Self {
            method,
            url,
            endpoint,
            body: None,
            limit: SMALL_BODY,
        }
    }

    pub(crate) fn json<T: serde::Serialize>(mut self, body: &T) -> Result<Self, ClientError> {
        let bytes = serde_json::to_vec(body).map_err(|_unencodable| ClientError::InvalidInput {
            what: "the request body does not encode",
        })?;
        self.body = Some(Bytes::from(bytes));
        Ok(self)
    }

    pub(crate) const fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

pub(crate) fn route(base: &str, path: &str, segments: &[&str]) -> Result<Url, ClientError> {
    if segments
        .iter()
        .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
    {
        return Err(ClientError::InvalidInput {
            what: "an id in the path is empty or relative",
        });
    }
    let mut url = resolve_instance_path(base, path).map_err(ClientError::Origin)?;
    if !segments.is_empty() {
        url.path_segments_mut()
            .map_err(|()| ClientError::InvalidInput {
                what: "the instance URL cannot carry a path",
            })?
            .extend(segments);
    }
    Ok(url)
}

pub(crate) async fn send_once(
    http: &reqwest::Client,
    call: &Call,
    jwt: Jwt,
) -> Result<Answer, ClientError> {
    let answer = exchange(http, call, &jwt).await?;
    if answer.status == 401 {
        // Why: discard-ok: a 401 with any body is a refusal.
        let envelope: ErrorEnvelope = serde_json::from_slice(&answer.body).unwrap_or_default();
        return Err(refusal(
            call.endpoint,
            plain_key(envelope.error_key.as_deref()),
        ));
    }
    Ok(answer)
}

async fn exchange(http: &reqwest::Client, call: &Call, jwt: &Jwt) -> Result<Answer, ClientError> {
    let endpoint = call.endpoint;
    let mut request = http
        .request(call.method.clone(), call.url.clone())
        .bearer_auth(jwt.expose())
        .header(ACCEPT, "application/json")
        .header(headers::CLIENT, HOST_ID)
        .timeout(TIMEOUT);
    if let Some(body) = &call.body {
        request = request
            .header(CONTENT_TYPE, "application/json")
            .body(body.clone());
    }
    let mut response = request
        .send()
        .await
        .map_err(|error| transport(endpoint, error))?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let limit_bytes = u64::try_from(call.limit).unwrap_or(u64::MAX);
    if response
        .content_length()
        .is_some_and(|length| length > limit_bytes)
    {
        return Err(too_large(endpoint, status));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| transport(endpoint, error))?
    {
        if body.len().saturating_add(chunk.len()) > call.limit {
            return Err(too_large(endpoint, status));
        }
        body.extend_from_slice(&chunk);
    }
    tracing::debug!(endpoint, status, bytes = body.len(), "brain route answered");
    Ok(Answer {
        status,
        headers,
        body,
    })
}

fn transport(endpoint: &'static str, error: reqwest::Error) -> ClientError {
    let error = error.without_url();
    tracing::debug!(
        endpoint,
        timeout = error.is_timeout(),
        connect = error.is_connect(),
        "brain request failed"
    );
    ClientError::Unreachable {
        endpoint,
        cause: TransportFailure::Request(error),
    }
}

fn too_large(endpoint: &'static str, status: u16) -> ClientError {
    protocol(
        endpoint,
        status,
        "the answer is larger than this route allows".to_owned(),
    )
}
