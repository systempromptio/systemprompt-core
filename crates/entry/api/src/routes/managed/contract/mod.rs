//! Uniform request bounds, problem details and generated `OpenAPI` contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
mod bounds;
pub mod openapi;
use axum::Json;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Problem details shared by authentication, extraction and domain failures.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
}

pub(crate) fn problem(status: StatusCode, detail: impl Into<String>) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, "application/problem+json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Json(Problem {
            kind: "about:blank".to_owned(),
            title: status
                .canonical_reason()
                .unwrap_or("Request failed")
                .to_owned(),
            status: status.as_u16(),
            detail: detail.into(),
        }),
    )
        .into_response()
}
pub async fn normalize(request: Request, next: Next) -> Response {
    let request = match bounds::validate(request).await {
        Ok(request) => request,
        Err(response) => return response,
    };
    let mut response = next.run(request).await;
    if (response.status().is_client_error() || response.status().is_server_error())
        && response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| !value.starts_with("application/problem+json"))
    {
        let status = response.status();
        let mut normalized = problem(
            status,
            match status {
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                    "Invalid request body, query or path parameters"
                },
                StatusCode::UNAUTHORIZED => "Valid authentication is required",
                StatusCode::FORBIDDEN => "This identity or browser origin is not authorized",
                StatusCode::NOT_FOUND => "The requested resource is unavailable",
                StatusCode::TOO_MANY_REQUESTS => {
                    "Request limit reached; retry after the indicated delay"
                },
                _ => "The operation could not be completed",
            },
        );
        for name in [header::WWW_AUTHENTICATE, header::RETRY_AFTER] {
            if let Some(value) = response.headers().get(&name) {
                normalized.headers_mut().insert(name, value.clone());
            }
        }
        return normalized;
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store"),
    );
    response
}
