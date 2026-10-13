//! HTTP projections of the SSO driver's outcomes. Public so a deployment can
//! serve the same flow from its own registered paths.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use systemprompt_oauth_issuance::sso::{
    CookieSpec, FinishOutcome, StartOutcome, binder_from_cookie_header,
};

use crate::routes::oauth::cookie::session_cookie_redirect;

pub fn into_response(outcome: FinishOutcome) -> Response {
    match outcome {
        FinishOutcome::Success {
            access_token,
            max_age_secs,
            return_to,
            clear_binder,
        } => with_cookie(
            session_cookie_redirect(&access_token, &return_to, max_age_secs),
            &clear_binder,
        ),
        FinishOutcome::Refused {
            login_path,
            reason,
            clear_binder,
        } => with_cookie(refused_redirect(&login_path, reason), &clear_binder),
    }
}

pub fn start_response(outcome: &StartOutcome) -> Response {
    with_cookie(
        Redirect::to(&outcome.redirect_url).into_response(),
        &outcome.binder_cookie,
    )
}

pub(super) fn refused_redirect(login_path: &str, reason: &str) -> Response {
    let separator = if login_path.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{login_path}{separator}sso={reason}")).into_response()
}

pub fn binder_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(binder_from_cookie_header)
        .map(str::to_owned)
}

fn with_cookie(mut response: Response, cookie: &CookieSpec) -> Response {
    if let Ok(value) = HeaderValue::from_str(&cookie.header_value()) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}
