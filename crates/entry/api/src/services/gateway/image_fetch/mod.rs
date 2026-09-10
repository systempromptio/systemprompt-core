//! Resolving caller-supplied image URLs to inline base64 for wires that
//! cannot carry a URL.
//!
//! Gemini's `generateContent` has no URL image part at all — `inlineData` or a
//! Files API handle are the only shapes it accepts — so the wire codec, which
//! is synchronous and has no HTTP client, can only downgrade a URL image to
//! text. This module does the fetch one layer up, in the dispatch pipeline,
//! before the body is built, and rewrites the canonical request in place so
//! the codec sees an image it can render.
//!
//! It is deliberately not a general-purpose fetcher. The URL comes from
//! whoever sent the inference request, so every fetch is guarded by `guard`,
//! bounded by a timeout, capped while the body streams, and accepted only if
//! the server declares a MIME type Gemini takes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod guard;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use systemprompt_models::net::{GuardedConnectError, trusted_http_hosts_from_env};

use super::protocol::canonical::{CanonicalContent, CanonicalRequest, ImageSource};

// Why: Gemini limits inline generateContent requests to 20 MB; base64 expands
// bytes by roughly 4/3.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

pub const ACCEPTED_MIME: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/heic",
    "image/heif",
];

const MAX_REDIRECTS: u8 = 3;
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// A caller-supplied image URL that could not be turned into inline data.
///
/// `caller_fault` separates "this URL was never going to work" — blocked host,
/// wrong content type, too large — from a transport failure reaching an
/// otherwise legitimate host, so the route layer can answer 400 or 502.
#[derive(Debug, thiserror::Error)]
#[error("image url {url} could not be inlined: {message}")]
pub struct ImageFetchFailed {
    pub url: String,
    pub message: String,
    pub caller_fault: bool,
}

/// Per-request bounds, so a test can point the fetcher at a loopback mock
/// without the process-wide trust list that production reads from the
/// environment.
#[derive(Debug, Clone)]
pub struct ImageFetchPolicy {
    pub timeout: std::time::Duration,
    pub max_bytes: usize,
    pub max_redirects: u8,
    pub trusted_hosts: Vec<String>,
}

impl Default for ImageFetchPolicy {
    fn default() -> Self {
        Self {
            timeout: FETCH_TIMEOUT,
            max_bytes: MAX_IMAGE_BYTES,
            max_redirects: MAX_REDIRECTS,
            trusted_hosts: trusted_http_hosts_from_env(),
        }
    }
}

/// Fetched bytes plus the MIME type the server declared for them.
#[derive(Debug, Clone)]
pub struct InlineImage {
    pub media_type: String,
    pub base64: String,
}

pub async fn inline_url_images(
    request: &mut CanonicalRequest,
    policy: &ImageFetchPolicy,
) -> Result<usize, ImageFetchFailed> {
    let mut count = 0usize;
    for message in &mut request.messages {
        for content in &mut message.content {
            let CanonicalContent::Image(ImageSource::Url { url, detail }) = content else {
                continue;
            };
            let fetched = fetch(url, policy).await?;
            *content = CanonicalContent::Image(ImageSource::Base64 {
                media_type: fetched.media_type,
                data: fetched.base64,
                detail: *detail,
            });
            count += 1;
        }
    }
    Ok(count)
}

pub async fn fetch(url: &str, policy: &ImageFetchPolicy) -> Result<InlineImage, ImageFetchFailed> {
    let fail = |message: String, caller_fault: bool| ImageFetchFailed {
        url: url.to_owned(),
        message,
        caller_fault,
    };
    tokio::time::timeout(policy.timeout, fetch_inner(url, policy))
        .await
        .map_or_else(
            |_| Err(fail(format!("fetch exceeded {:?}", policy.timeout), false)),
            |result| result.map_err(|(message, caller_fault)| fail(message, caller_fault)),
        )
}

type FetchError = (String, bool);

async fn fetch_inner(url: &str, policy: &ImageFetchPolicy) -> Result<InlineImage, FetchError> {
    let checked = guard::checked_url(url, &policy.trusted_hosts).map_err(|e| (e, true))?;
    let client = guard::client(policy).map_err(|e| (e, false))?;
    let response = client
        .get(checked)
        .send()
        .await
        .map_err(|e| describe_send_error(&e, policy))?;
    read_image(response, policy).await
}

// Why: reqwest reports a refused redirect and a timeout as generic send
// failures; the guard's verdict and the deadline sit in the source chain.
fn describe_send_error(error: &reqwest::Error, policy: &ImageFetchPolicy) -> FetchError {
    if error.is_timeout() {
        return (format!("fetch exceeded {:?}", policy.timeout), false);
    }
    let mut source = std::error::Error::source(error);
    while let Some(inner) = source {
        if let Some(guarded) = inner.downcast_ref::<GuardedConnectError>() {
            return match guarded {
                GuardedConnectError::RedirectRefused { .. }
                | GuardedConnectError::TooManyRedirects(_) => {
                    (format!("redirect rejected: {guarded}"), true)
                },
                GuardedConnectError::Unresolvable(_)
                | GuardedConnectError::BlockedAddress { .. } => {
                    (format!("host rejected: {guarded}"), true)
                },
            };
        }
        source = inner.source();
    }
    (format!("request failed: {error}"), error.is_redirect())
}

async fn read_image(
    mut response: reqwest::Response,
    policy: &ImageFetchPolicy,
) -> Result<InlineImage, FetchError> {
    let status = response.status();
    if !status.is_success() {
        return Err((format!("host returned {status}"), true));
    }
    let media_type = declared_mime(&response)?;
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| (format!("read failed: {e}"), false))?
    {
        if body.len() + chunk.len() > policy.max_bytes {
            return Err((format!("larger than {} bytes", policy.max_bytes), true));
        }
        body.extend_from_slice(&chunk);
    }
    if body.is_empty() {
        return Err(("empty response body".to_owned(), true));
    }
    Ok(InlineImage {
        media_type,
        base64: BASE64.encode(&body),
    })
}

fn declared_mime(response: &reqwest::Response) -> Result<String, FetchError> {
    let raw = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ("no content-type".to_owned(), true))?;
    let mime = raw
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if ACCEPTED_MIME.contains(&mime.as_str()) {
        return Ok(mime);
    }
    Err((
        format!("content-type {mime} is not an inlineable image"),
        true,
    ))
}
