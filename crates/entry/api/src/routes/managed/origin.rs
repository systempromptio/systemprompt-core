//! Cookie-authenticated mutations require a matching origin; the bridge
//! authenticates with a device bearer credential and sends no cookie.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_runtime::AppContext;

pub async fn protect(State(ctx): State<AppContext>, request: Request, next: Next) -> Response {
    if !request.method().is_safe() && request.headers().contains_key(header::COOKIE) {
        let expected = url::Url::parse(&ctx.config().api_external_url)
            .ok()
            .map(|url| url.origin().ascii_serialization());
        let actual = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok());
        if expected.as_deref().is_none() || actual != expected.as_deref() {
            return (
                StatusCode::FORBIDDEN,
                "Same-origin browser request required",
            )
                .into_response();
        }
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store"),
    );
    response
}
