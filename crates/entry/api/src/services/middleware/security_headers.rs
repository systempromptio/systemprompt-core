//! Security response headers (CSP, frame options, HSTS).
//!
//! A route that sets `X-Content-Type-Options` itself (user content) keeps
//! its value; every other response gets the configured one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_extension::FrameOptionsOverride;
use systemprompt_manifest::profile::SecurityHeadersConfig;

pub async fn inject_security_headers(
    config: SecurityHeadersConfig,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let frame_override = response.extensions().get::<FrameOptionsOverride>().copied();
    let headers = response.headers_mut();

    if let Ok(value) = HeaderValue::from_str(&config.hsts) {
        headers.insert("strict-transport-security", value);
    }

    if let Some(FrameOptionsOverride(frame_options)) = frame_override {
        match frame_options.header_value() {
            Some(value) => {
                headers.insert("x-frame-options", HeaderValue::from_static(value));
            },
            None => {
                headers.remove("x-frame-options");
            },
        }
        let existing = headers
            .get("content-security-policy")
            .and_then(|value| value.to_str().ok());
        let policy = with_frame_ancestors(existing, frame_options.frame_ancestors());
        if let Ok(value) = HeaderValue::from_str(&policy) {
            headers.insert("content-security-policy", value);
        }
    } else {
        match config.frame_options.header_value() {
            Some(value) => {
                headers.insert("x-frame-options", HeaderValue::from_static(value));
            },
            None => {
                headers.remove("x-frame-options");
            },
        }
    }

    if !headers.contains_key("x-content-type-options")
        && let Ok(value) = HeaderValue::from_str(&config.content_type_options)
    {
        headers.insert("x-content-type-options", value);
    }

    headers.insert(
        "referrer-policy",
        HeaderValue::from_static(config.referrer_policy.header_value()),
    );

    if let Ok(value) = HeaderValue::from_str(&config.permissions_policy) {
        headers.insert("permissions-policy", value);
    }

    if let Some(ref csp) = config.content_security_policy
        && frame_override.is_none()
        && let Ok(value) = HeaderValue::from_str(csp)
    {
        headers.insert("content-security-policy", value);
    }

    response
}

fn with_frame_ancestors(existing: Option<&str>, ancestors: &str) -> String {
    let directive = format!("frame-ancestors {ancestors}");
    let kept: Vec<&str> = existing
        .into_iter()
        .flat_map(|policy| policy.split(';'))
        .map(str::trim)
        .filter(|part| {
            !part.is_empty()
                && !part
                    .split_ascii_whitespace()
                    .next()
                    .is_some_and(|name| name.eq_ignore_ascii_case("frame-ancestors"))
        })
        .collect();
    if kept.is_empty() {
        return directive;
    }
    format!("{}; {directive}", kept.join("; "))
}
