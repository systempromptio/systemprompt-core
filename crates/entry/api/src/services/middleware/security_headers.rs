use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_models::profile::SecurityHeadersConfig;

pub async fn inject_security_headers(
    config: SecurityHeadersConfig,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    if let Ok(value) = HeaderValue::from_str(&config.hsts) {
        headers.insert("strict-transport-security", value);
    }

    if let Ok(value) = HeaderValue::from_str(&config.frame_options) {
        headers.insert("x-frame-options", value);
    }

    if let Ok(value) = HeaderValue::from_str(&config.content_type_options) {
        headers.insert("x-content-type-options", value);
    }

    if let Ok(value) = HeaderValue::from_str(&config.referrer_policy) {
        headers.insert("referrer-policy", value);
    }

    if let Ok(value) = HeaderValue::from_str(&config.permissions_policy) {
        headers.insert("permissions-policy", value);
    }

    if let Some(ref csp) = config.content_security_policy {
        if let Ok(value) = HeaderValue::from_str(csp) {
            headers.insert("content-security-policy", value);
        }
    }

    response
}
