//! Transport shared by every route.
//!
//! One [`HttpClient`] (rustls, a neutral User-Agent the caller names, a connect
//! timeout, redirects never followed) is shared by every [`crate::client::SpClient`].
//! Each route sets its own whole-answer timeout and body limit; an answer over
//! its limit is refused before it is buffered. A 3xx is a protocol error:
//! following it could carry a credential to another origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{RequestBuilder, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::client::error::{ClientError, TransportFailure, error_key, excerpt};

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const SMALL_BODY: usize = 256 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

const SESSION_MISMATCH_KEY: &str = "session_mismatch";

/// A reqwest client built for instance calls: it never follows a redirect.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
}

/// The HTTP client could not be built (a User-Agent that is not a header value,
/// or no TLS backend).
#[derive(Debug, thiserror::Error)]
#[error("the HTTP client could not be built")]
pub struct HttpClientError(#[source] reqwest::Error);

impl HttpClient {
    pub fn new(user_agent: &str) -> Result<Self, HttpClientError> {
        let inner = reqwest::Client::builder()
            .user_agent(user_agent)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(HttpClientError)?;
        Ok(Self { inner })
    }

    #[must_use]
    pub const fn reqwest(&self) -> &reqwest::Client {
        &self.inner
    }
}

#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: StatusCode,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

pub(crate) async fn exchange(
    endpoint: &'static str,
    request: RequestBuilder,
    timeout: Duration,
    limit: usize,
) -> Result<Answer, ClientError> {
    let mut response = request
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| transport(endpoint, error))?;
    let status = response.status();
    let headers = response.headers().clone();
    let limit_bytes = u64::try_from(limit).unwrap_or(u64::MAX);
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
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(too_large(endpoint, status));
        }
        body.extend_from_slice(&chunk);
    }
    tracing::debug!(
        endpoint,
        status = status.as_u16(),
        bytes = body.len(),
        "instance answered"
    );
    Ok(Answer {
        status,
        headers,
        body,
    })
}

pub(crate) fn decode<T: DeserializeOwned>(
    endpoint: &'static str,
    answer: &Answer,
) -> Result<T, ClientError> {
    serde_json::from_slice(&answer.body)
        .map_err(|error| decode_error(endpoint, Some(answer.status.as_u16()), &error))
}

pub(crate) fn decode_error(
    endpoint: &'static str,
    status: Option<u16>,
    error: &serde_json::Error,
) -> ClientError {
    ClientError::Protocol {
        endpoint,
        status,
        what: format!(
            "the answer does not decode ({:?} error at line {}, column {})",
            error.classify(),
            error.line(),
            error.column()
        ),
        detail: None,
    }
}

pub(crate) fn failure(endpoint: &'static str, answer: &Answer) -> ClientError {
    let rejection = Rejection::read(&answer.body);
    let status = answer.status.as_u16();
    match status {
        401 if rejection.error_key.as_deref() == Some(SESSION_MISMATCH_KEY) => {
            ClientError::SessionMismatch { endpoint }
        },
        401 => ClientError::Unauthorized {
            endpoint,
            error_key: rejection.error_key,
        },
        429 => ClientError::RateLimited {
            endpoint,
            retry_after: retry_after(&answer.headers, Utc::now()),
        },
        500.. => ClientError::Unreachable {
            endpoint,
            cause: TransportFailure::Status { status },
        },
        300..=399 => ClientError::Protocol {
            endpoint,
            status: Some(status),
            what: "the instance answered with a redirect, which is never followed".to_owned(),
            detail: None,
        },
        _ => ClientError::Protocol {
            endpoint,
            status: Some(status),
            what: format!("unexpected status {status}"),
            detail: rejection.detail,
        },
    }
}

pub(crate) fn protocol(endpoint: &'static str, status: Option<u16>, what: &str) -> ClientError {
    ClientError::Protocol {
        endpoint,
        status,
        what: what.to_owned(),
        detail: None,
    }
}

fn transport(endpoint: &'static str, error: reqwest::Error) -> ClientError {
    let error = error.without_url();
    tracing::debug!(
        endpoint,
        timeout = error.is_timeout(),
        connect = error.is_connect(),
        "instance request failed"
    );
    ClientError::Unreachable {
        endpoint,
        cause: TransportFailure::Request(error),
    }
}

fn too_large(endpoint: &'static str, status: StatusCode) -> ClientError {
    protocol(
        endpoint,
        Some(status.as_u16()),
        "the answer is larger than this route allows",
    )
}

pub(crate) fn retry_after(headers: &HeaderMap, now: DateTime<Utc>) -> Option<Duration> {
    let raw = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) {
        return raw.parse::<u64>().ok().map(Duration::from_secs);
    }
    let at = DateTime::parse_from_rfc2822(raw).ok()?.with_timezone(&Utc);
    Some((at - now).to_std().unwrap_or(Duration::ZERO))
}

struct Rejection {
    error_key: Option<String>,
    detail: Option<String>,
}

impl Rejection {
    fn read(body: &[u8]) -> Self {
        #[derive(Deserialize)]
        struct Body {
            #[serde(default)]
            error_key: Option<String>,
            #[serde(default)]
            message: Option<String>,
            #[serde(default)]
            detail: Option<String>,
            #[serde(default)]
            error_description: Option<String>,
        }
        match serde_json::from_slice::<Body>(body) {
            Ok(parsed) => Self {
                error_key: error_key(parsed.error_key.as_deref()),
                detail: parsed
                    .detail
                    .or(parsed.message)
                    .or(parsed.error_description)
                    .as_deref()
                    .and_then(excerpt),
            },
            Err(_undecodable) => Self {
                error_key: None,
                detail: None,
            },
        }
    }
}
