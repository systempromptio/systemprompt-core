use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::any;
use axum::Router;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub fn create_proxy_router(target_host: &str, target_port: u16) -> Router {
    let target_url = format!("http://{target_host}:{target_port}");

    Router::new().fallback(any(move |req: Request| {
        let url = target_url.clone();
        async move { forward_request(req, url).await }
    }))
}

async fn forward_request(
    req: Request,
    target_url: String,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let path = req.uri().path();
    let query = req
        .uri()
        .query()
        .map_or_else(String::new, |q| format!("?{q}"));
    let full_url = format!("{target_url}{path}{query}");

    let client = reqwest::Client::new();

    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut proxied = client.request(method, &full_url);

    for (key, value) in req.headers() {
        if key != "host" {
            proxied = proxied.header(key.as_str(), value.as_bytes());
        }
    }

    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !body_bytes.is_empty() {
        proxied = proxied.body(body_bytes.to_vec());
    }

    let response = proxied.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let status_code = response.status().as_u16();
    let status = StatusCode::from_u16(status_code).map_err(|_| StatusCode::BAD_GATEWAY)?;

    let body = response
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok((status, body))
}

pub fn create_load_balanced_proxy(targets: &[(String, u16)]) -> Router {
    let target_urls: Vec<String> = targets
        .iter()
        .map(|(host, port)| format!("http://{host}:{port}"))
        .collect();

    let counter = Arc::new(AtomicUsize::new(0));

    Router::new().fallback(any(move |req: Request| {
        let urls = target_urls.clone();
        let cnt = Arc::clone(&counter);
        async move {
            let index = cnt.fetch_add(1, Ordering::Relaxed) % urls.len();
            let url = urls[index].clone();
            forward_request(req, url).await
        }
    }))
}
