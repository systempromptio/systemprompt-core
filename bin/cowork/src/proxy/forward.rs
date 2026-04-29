use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::{BodyExt, BodyStream, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::{HeaderMap, Request, Response, StatusCode};
use systemprompt_identifiers::ValidatedUrl;
use thiserror::Error;

use crate::{auth, config};

const REFRESH_THRESHOLD_SECS: u64 = 300;

const HOP_BY_HOP: &[&str] = &[
    "host",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "content-length",
    "authorization",
    "x-api-key",
];

pub type ProxyBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

#[derive(Debug, Error)]
pub enum ForwardError {
    #[error("authentication unavailable: {0}")]
    Auth(String),
    #[error("invalid request method {method}: {source}")]
    BadMethod {
        method: String,
        #[source]
        source: http::method::InvalidMethod,
    },
    #[error("invalid header value: {0}")]
    BadHeader(String),
    #[error("upstream request failed: {0}")]
    Upstream(#[from] reqwest::Error),
    #[error("response build failed: {0}")]
    BuildResponse(#[from] http::Error),
}

pub type ForwardResult<T> = Result<T, ForwardError>;

#[tracing::instrument(level = "debug", skip(req, client, gateway_base), fields(method = %req.method(), path = %req.uri().path()))]
pub async fn forward(
    req: Request<Incoming>,
    client: reqwest::Client,
    gateway_base: &ValidatedUrl,
) -> ForwardResult<Response<ProxyBody>> {
    let token = mint_token().await?;

    let (parts, body) = req.into_parts();
    let url = build_upstream_url(gateway_base, &parts.uri);

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).map_err(|e| {
        ForwardError::BadMethod {
            method: parts.method.to_string(),
            source: e,
        }
    })?;

    let upstream_headers = build_upstream_headers(&parts.headers, token.token.expose())?;
    let upstream_body = reqwest::Body::wrap_stream(
        BodyStream::new(body)
            .map_ok(|frame: Frame<Bytes>| frame.into_data().unwrap_or_default())
            .map_err(|e| std::io::Error::other(e.to_string())),
    );

    let upstream_response = client
        .request(method, &url)
        .headers(upstream_headers)
        .body(upstream_body)
        .send()
        .await?;

    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    if status.is_success() {
        tracing::debug!(upstream_status = status.as_u16(), "upstream forwarded");
    } else {
        tracing::warn!(upstream_status = status.as_u16(), %url, "upstream non-2xx");
    }

    let mut response_builder = Response::builder().status(status);
    if let Some(headers_mut) = response_builder.headers_mut() {
        copy_response_headers(upstream_response.headers(), headers_mut);
    }

    let upstream_stream = upstream_response
        .bytes_stream()
        .map_ok(Frame::data)
        .map_err(|e| std::io::Error::other(e.to_string()));
    let body: ProxyBody = StreamBody::new(upstream_stream).boxed();

    Ok(response_builder.body(body)?)
}

async fn mint_token() -> ForwardResult<auth::types::HelperOutput> {
    tokio::task::spawn_blocking(|| {
        let cfg = config::load();
        auth::read_or_refresh(&cfg, REFRESH_THRESHOLD_SECS)
    })
    .await
    .map_err(|e| ForwardError::Auth(format!("auth task join: {e}")))?
    .ok_or_else(|| ForwardError::Auth("no JWT available — sign in via cowork GUI".to_string()))
}

fn build_upstream_url(gateway_base: &ValidatedUrl, uri: &http::Uri) -> String {
    let path_and_query = uri.path_and_query().map_or("/", |p| p.as_str());
    let separator = if path_and_query.starts_with('/') {
        ""
    } else {
        "/"
    };
    format!(
        "{base}{separator}{path_and_query}",
        base = gateway_base.as_str().trim_end_matches('/'),
    )
}

fn build_upstream_headers(
    src: &HeaderMap,
    bearer: &str,
) -> ForwardResult<reqwest::header::HeaderMap> {
    let mut headers = reqwest::header::HeaderMap::with_capacity(src.len() + 2);
    copy_request_headers(src, &mut headers);

    let bearer = reqwest::header::HeaderValue::try_from(format!("Bearer {bearer}"))
        .map_err(|e| ForwardError::BadHeader(format!("authorization: {e}")))?;
    headers.insert(reqwest::header::AUTHORIZATION, bearer);
    headers.insert(
        reqwest::header::HeaderName::from_static("x-cowork-proxied"),
        reqwest::header::HeaderValue::from_static("1"),
    );

    Ok(headers)
}

fn copy_request_headers(src: &HeaderMap, dest: &mut reqwest::header::HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.append(name, value);
    }
}

fn copy_response_headers(src: &reqwest::header::HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            hyper::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            hyper::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.insert(name, value);
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}

#[must_use]
pub fn is_client_disconnect(err: &ForwardError) -> bool {
    matches!(
        err,
        ForwardError::Upstream(e)
            if e.is_request() && e.to_string().contains("connection closed")
    )
}

const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<ForwardResult<Response<ProxyBody>>>();
};
