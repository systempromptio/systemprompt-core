//! The browser session cookie and the redirect that sets it, shared by the
//! OAuth browser-client callback and the SSO callbacks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};

pub fn session_cookie_redirect(
    access_token: &str,
    destination: &str,
    max_age_secs: i64,
) -> Response {
    let cookie = format!(
        "access_token={access_token}; Path=/; HttpOnly; Secure; SameSite=Strict; \
         Max-Age={max_age_secs}"
    );

    let mut response = Redirect::to(destination).into_response();
    if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie_value);
    }

    response
}
