//! One bounded exchange with an instance route, the bearer attached.
//!
//! The URL is resolved on the instance's own origin, redirects are never
//! followed (the shared `systemprompt_device::client::HttpClient` refuses them), an answer is
//! read up to its route's limit and refused past it, and a transport error is
//! reported without its URL. A 401 is turned into the client's refusal so
//! `systemprompt_device::client::with_jwt` mints a fresh token and tries once more.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use reqwest::Method;
use reqwest::header::{ACCEPT, CONTENT_TYPE, RETRY_AFTER};
use serde::Deserialize;
use systemprompt_device::client::{ClientError, TransportFailure};
use sp_protocol::wire::headers;
use sp_protocol::{HOST_ID, Jwt};
use url::Url;

const TIMEOUT: Duration = Duration::from_secs(30);

const SESSION_MISMATCH_KEY: &str = "session_mismatch";

const ERROR_KEY_MAX: usize = 64;

#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) method: Method,
    pub(crate) url: Url,
    pub(crate) endpoint: &'static str,
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) limit: usize,
}

#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) retry_after: Option<Duration>,
    pub(crate) body: Vec<u8>,
}

pub(crate) async fn send_once(
    http: &reqwest::Client,
    call: &Call,
    jwt: Jwt,
) -> Result<Answer, ClientError> {
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
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    let limit = u64::try_from(call.limit).unwrap_or(u64::MAX);
    if response
        .content_length()
        .is_some_and(|length| length > limit)
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
    tracing::debug!(
        endpoint,
        status,
        bytes = body.len(),
        "notifications route answered"
    );
    if status == 401 {
        return Err(refusal(endpoint, &body));
    }
    Ok(Answer {
        status,
        retry_after,
        body,
    })
}

fn refusal(endpoint: &'static str, body: &[u8]) -> ClientError {
    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        error_key: Option<String>,
    }
    let error_key = serde_json::from_slice::<Body>(body)
        .ok()
        .and_then(|parsed| parsed.error_key)
        .filter(|key| is_plain_key(key));
    if error_key.as_deref() == Some(SESSION_MISMATCH_KEY) {
        ClientError::SessionMismatch { endpoint }
    } else {
        ClientError::Unauthorized {
            endpoint,
            error_key,
        }
    }
}

fn is_plain_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= ERROR_KEY_MAX
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

fn transport(endpoint: &'static str, error: reqwest::Error) -> ClientError {
    let error = error.without_url();
    tracing::debug!(
        endpoint,
        timeout = error.is_timeout(),
        connect = error.is_connect(),
        "notifications request failed"
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
