//! The one builder for the `access_token` browser cookie.
//!
//! Every `Set-Cookie` for the session token — issue and clear — comes from
//! here, so the attributes cannot drift between call sites: always `Path=/`,
//! `HttpOnly` and `Secure`. Browsers treat `http://localhost` as a secure
//! context, so `Secure` costs local development nothing, while a clear issued
//! with different attributes than the set would fail to replace it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderValue;
use axum::http::header::InvalidHeaderValue;
use systemprompt_security::CookieExtractor;

/// The `SameSite` policy a session cookie is issued under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
}

impl SameSite {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "Strict",
            Self::Lax => "Lax",
        }
    }
}

pub fn session_cookie(
    token: &str,
    max_age_secs: i64,
    same_site: SameSite,
) -> Result<HeaderValue, InvalidHeaderValue> {
    HeaderValue::from_str(&format!(
        "{}={token}; Path=/; HttpOnly; Secure; SameSite={}; Max-Age={max_age_secs}",
        CookieExtractor::DEFAULT_COOKIE_NAME,
        same_site.as_str(),
    ))
}

#[must_use]
pub const fn cleared_session_cookie() -> HeaderValue {
    HeaderValue::from_static("access_token=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}
