//! The one builder for the `access_token` browser cookie.
//!
//! Every `Set-Cookie` for the session token — issue and clear — comes from a
//! [`SessionCookie`], so the attributes cannot drift between call sites:
//! always `Path=/` and `HttpOnly`, and `Secure` exactly when the server's
//! configured `api_external_url` is `https`. The request never decides: a
//! deployment that is served over https always sets `Secure`, and one that is
//! deliberately served over plain http (a LAN address, a container hostname)
//! still gets a cookie the browser will store. An external URL that does not
//! parse sets `Secure`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderValue;
use axum::http::header::InvalidHeaderValue;
use systemprompt_manifest::Config;
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

/// Session-cookie attributes derived once from the server configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionCookie {
    secure: bool,
}

impl SessionCookie {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self::for_external_url(&config.api_external_url)
    }

    #[must_use]
    pub fn for_external_url(api_external_url: &str) -> Self {
        let secure = url::Url::parse(api_external_url).map_or(true, |url| url.scheme() != "http");
        Self { secure }
    }

    #[must_use]
    pub const fn is_secure(self) -> bool {
        self.secure
    }

    const fn secure_attr(self) -> &'static str {
        if self.secure { "; Secure" } else { "" }
    }

    pub fn issue(
        self,
        token: &str,
        max_age_secs: i64,
        same_site: SameSite,
    ) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::from_str(&format!(
            "{}={token}; Path=/; HttpOnly{}; SameSite={}; Max-Age={max_age_secs}",
            CookieExtractor::DEFAULT_COOKIE_NAME,
            self.secure_attr(),
            same_site.as_str(),
        ))
    }

    #[must_use]
    pub const fn cleared(self) -> HeaderValue {
        HeaderValue::from_static(if self.secure {
            "access_token=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0"
        } else {
            "access_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        })
    }
}
