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
//! whoever sent the inference request, so every fetch is guarded by [`guard`],
//! bounded by a timeout, capped while the body streams, and accepted only if
//! the server declares a MIME type Gemini takes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod guard;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, trusted_http_hosts_from_env};

use super::protocol::canonical::{CanonicalContent, CanonicalRequest, ImageSource};

// Why: Gemini caps a whole `generateContent` request at 20 MB inline, and
// base64 inflates by 4/3. A 5 MiB ceiling per image leaves a conversation room
// for several images plus its text inside that budget, and is already well
// above what any real photograph in a prompt weighs.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

// Why: the shapes Gemini documents for `inlineData`. A server declaring
// anything else is either not serving an image or serving one the model cannot
// decode; both are failures, not things to inline and hope.
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

// Why: redirects are followed by hand so every hop is re-checked against the
// guard. reqwest's own policy would resolve a redirect to 169.254.169.254
// internally, and the only URL this code ever saw would be the innocent one.
fn client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

// Why: the first failure aborts and returns rather than skipping the image. An
// image the caller asked the model to look at is part of the prompt, and
// answering about a prompt that quietly lost one of its inputs is the defect
// this module exists to remove.
pub async fn inline_url_images(
    request: &mut CanonicalRequest,
    policy: &ImageFetchPolicy,
) -> Result<usize, ImageFetchFailed> {
    let mut count = 0_usize;
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

// Why: the timeout wraps guard, connect, redirects and body read together, so
// a host that stalls each step just under a per-step budget still cannot hold
// the inference request open.
pub async fn fetch(url: &str, policy: &ImageFetchPolicy) -> Result<InlineImage, ImageFetchFailed> {
    let fail = |message: String, caller_fault: bool| ImageFetchFailed {
        url: url.to_owned(),
        message,
        caller_fault,
    };
    match tokio::time::timeout(policy.timeout, fetch_inner(url, policy)).await {
        Ok(result) => result.map_err(|(message, caller_fault)| fail(message, caller_fault)),
        Err(_) => Err(fail(format!("fetch exceeded {:?}", policy.timeout), false)),
    }
}

type FetchError = (String, bool);

async fn fetch_inner(url: &str, policy: &ImageFetchPolicy) -> Result<InlineImage, FetchError> {
    let mut next = guard::checked_url(url, &policy.trusted_hosts)
        .await
        .map_err(|e| (e, true))?;
    for _ in 0..=policy.max_redirects {
        let response = client()
            .get(next.clone())
            .send()
            .await
            .map_err(|e| (format!("request failed: {e}"), false))?;
        if let Some(location) = redirect_target(&response) {
            let joined = next
                .join(&location)
                .map_err(|e| (format!("invalid redirect target: {e}"), true))?;
            next = guard::checked_url(joined.as_str(), &policy.trusted_hosts)
                .await
                .map_err(|e| (format!("redirect rejected: {e}"), true))?;
            continue;
        }
        return read_image(response, policy).await;
    }
    Err((
        format!("more than {} redirects", policy.max_redirects),
        true,
    ))
}

fn redirect_target(response: &reqwest::Response) -> Option<String> {
    if !response.status().is_redirection() {
        return None;
    }
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned)
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
    // Why: the cap is enforced chunk by chunk rather than on the finished body,
    // so a host advertising nothing and sending gigabytes is dropped after the
    // first 5 MiB instead of being buffered whole and measured afterwards.
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
