//! HTTP forwarding to local MCP server ports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::any;
use futures::TryStreamExt;

pub fn create_proxy_router(target_host: &str, target_port: u16) -> Router {
    let target_url = format!("http://{target_host}:{target_port}");

    Router::new().fallback(any(move |req: Request| {
        let url = target_url.clone();
        async move { forward_request(req, url).await }
    }))
}

fn shared_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

// Why: hop-by-hop headers are connection-scoped (RFC 9110 §7.6.1); copying
// them onto the reconstructed response corrupts framing on the client leg.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

async fn forward_request(req: Request, target_url: String) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let query = req
        .uri()
        .query()
        .map_or_else(String::new, |q| format!("?{q}"));
    let full_url = format!("{target_url}{path}{query}");

    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .map_err(|_e| StatusCode::BAD_REQUEST)?;

    let mut proxied = shared_client().request(method, &full_url);

    for (key, value) in req.headers() {
        if key != "host" {
            proxied = proxied.header(key.as_str(), value.as_bytes());
        }
    }

    let body_bytes = axum::body::to_bytes(req.into_body(), crate::MAX_REQUEST_BODY_BYTES)
        .await
        .map_err(|_e| StatusCode::PAYLOAD_TOO_LARGE)?;

    if !body_bytes.is_empty() {
        proxied = proxied.body(body_bytes.to_vec());
    }

    let upstream = proxied.send().await.map_err(|_e| StatusCode::BAD_GATEWAY)?;

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).map_err(|_e| StatusCode::BAD_GATEWAY)?;

    let mut response = Response::builder().status(status);
    if let Some(headers) = response.headers_mut() {
        for (key, value) in upstream.headers() {
            let name = key.as_str();
            if HOP_BY_HOP.contains(&name) {
                continue;
            }
            if let (Ok(name), Ok(value)) = (
                http::HeaderName::from_bytes(name.as_bytes()),
                http::HeaderValue::from_bytes(value.as_bytes()),
            ) {
                headers.insert(name, value);
            }
        }
    }

    let stream = upstream.bytes_stream().map_err(std::io::Error::other);
    response
        .body(Body::from_stream(stream))
        .map_err(|_e| StatusCode::BAD_GATEWAY)
}
