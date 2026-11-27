//! The request layer under [`crate::DriveClient`] and the sync feed: one
//! bounded exchange with a bearer.
//!
//! The URL is always resolved on the account's own origin
//! (`sp_protocol::origin`), redirects are never followed (the
//! shared [`sp_client::HttpClient`] refuses them), every answer is read up to
//! its route's limit and refused past it, and a transport error is reported
//! without its URL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use bytes::Bytes;
use reqwest::Method;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap};
use serde::Deserialize;
use sp_client::{ClientError, TransportFailure};
use sp_protocol::ids::ContextId;
use sp_protocol::origin::resolve_instance_path;
use sp_protocol::wire::headers;
use sp_protocol::{HOST_ID, Jwt};
use url::Url;

use crate::error::plain_key;

mod form;

use form::form;
pub(crate) use form::{FormPart, Payload};

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub(crate) const SMALL_BODY: usize = 1024 * 1024;

const SESSION_MISMATCH_KEY: &str = "session_mismatch";

#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

impl Answer {
    pub(crate) fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }
}

#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) method: Method,
    pub(crate) url: Url,
    pub(crate) endpoint: &'static str,
    pub(crate) payload: Payload,
    pub(crate) headers: Vec<(&'static str, String)>,
    pub(crate) accept: &'static str,
    pub(crate) timeout: Duration,
    pub(crate) limit: usize,
}

impl Call {
    pub(crate) const fn new(method: Method, url: Url, endpoint: &'static str) -> Self {
        Self {
            method,
            url,
            endpoint,
            payload: Payload::Empty,
            headers: Vec::new(),
            accept: "application/json",
            timeout: DEFAULT_TIMEOUT,
            limit: SMALL_BODY,
        }
    }

    pub(crate) fn json<T: serde::Serialize>(mut self, body: &T) -> Result<Self, ClientError> {
        let bytes = serde_json::to_vec(body).map_err(|_unencodable| ClientError::InvalidInput {
            what: "the request body does not encode",
        })?;
        self.payload = Payload::Json(Bytes::from(bytes));
        Ok(self)
    }

    pub(crate) fn payload(mut self, payload: Payload) -> Self {
        self.payload = payload;
        self
    }

    pub(crate) fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    pub(crate) fn context(self, context_id: Option<&ContextId>) -> Self {
        match context_id {
            Some(id) => self.header(headers::CONTEXT_ID, id.as_str()),
            None => self,
        }
    }

    pub(crate) const fn accept(mut self, accept: &'static str) -> Self {
        self.accept = accept;
        self
    }

    pub(crate) const fn limits(mut self, timeout: Duration, limit: usize) -> Self {
        self.timeout = timeout;
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
    let answer = exchange(http, call, jwt.expose()).await?;
    if answer.status == 401 {
        return Err(refusal(call.endpoint, &answer.body));
    }
    Ok(answer)
}

pub(crate) async fn exchange(
    http: &reqwest::Client,
    call: &Call,
    bearer: &str,
) -> Result<Answer, ClientError> {
    let endpoint = call.endpoint;
    let mut request = http
        .request(call.method.clone(), call.url.clone())
        .bearer_auth(bearer)
        .header(ACCEPT, call.accept)
        .header(headers::CLIENT, HOST_ID)
        .timeout(call.timeout);
    for (name, value) in &call.headers {
        request = request.header(*name, value.as_str());
    }
    request = match &call.payload {
        Payload::Empty => request,
        Payload::Json(body) => request
            .header(CONTENT_TYPE, "application/json")
            .body(body.clone()),
        Payload::Raw(body) => request
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(body.clone()),
        Payload::Multipart(parts) => request.multipart(form(parts)?),
    };
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
    tracing::debug!(endpoint, status, bytes = body.len(), "drive route answered");
    Ok(Answer {
        status,
        headers,
        body,
    })
}

pub(crate) fn refusal(endpoint: &'static str, body: &[u8]) -> ClientError {
    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        error_key: Option<String>,
    }
    let error_key = serde_json::from_slice::<Body>(body)
        .ok()
        .and_then(|parsed| plain_key(parsed.error_key.as_deref()));
    if error_key.as_deref() == Some(SESSION_MISMATCH_KEY) {
        ClientError::SessionMismatch { endpoint }
    } else {
        ClientError::Unauthorized {
            endpoint,
            error_key,
        }
    }
}

fn transport(endpoint: &'static str, error: reqwest::Error) -> ClientError {
    let error = error.without_url();
    tracing::debug!(
        endpoint,
        timeout = error.is_timeout(),
        connect = error.is_connect(),
        "drive request failed"
    );
    ClientError::Unreachable {
        endpoint,
        cause: TransportFailure::Request(error),
    }
}

fn too_large(endpoint: &'static str, status: u16) -> ClientError {
    ClientError::Protocol {
        endpoint,
        status: Some(status),
        what: "the answer is larger than this route allows".to_owned(),
        detail: None,
    }
}
