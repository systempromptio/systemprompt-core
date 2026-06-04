//! Request-derived base URL for OAuth discovery responses.
//!
//! RFC 9728 implementations identify themselves coherently from the host the
//! client actually dialled. A single gateway reachable via both `127.0.0.1`
//! and `localhost` must echo whichever the client used in every URL it
//! returns (`issuer`, `authorization_endpoint`, `token_endpoint`, `resource`…),
//! otherwise the client's RFC 8707 `resource` indicator won't round-trip
//! against the configured `api_external_url` origin.
//!
//! [`RequestBaseUrl`] is an axum extractor that resolves
//! `scheme://host[:port]` from the incoming request, validating the host
//! against a small allowlist seeded from `api_external_url`. On allowlist
//! miss or missing/invalid header it falls back to `api_external_url` — the
//! gateway never advertises a hostname an attacker fabricated via Host
//! header injection.

use axum::extract::FromRequestParts;
use http::request::Parts;
use http::{StatusCode, header};
use systemprompt_models::Config;

#[derive(Debug, Clone)]
pub struct RequestBaseUrl {
    base: String,
    origin: url::Origin,
}

impl RequestBaseUrl {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.base
    }

    #[must_use]
    pub const fn origin(&self) -> &url::Origin {
        &self.origin
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.base
    }
}

fn is_loopback_host(host: &str) -> bool {
    let bare = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    bare == "localhost" || bare == "127.0.0.1" || bare == "[::1]" || bare == "::1"
}

fn host_in_allowlist(candidate_host: &str, configured: &url::Url) -> bool {
    let candidate_bare = candidate_host
        .rsplit_once(':')
        .map_or(candidate_host, |(h, _)| h)
        .to_ascii_lowercase();
    let configured_host = configured.host_str().unwrap_or("").to_ascii_lowercase();

    if candidate_bare == configured_host {
        return true;
    }
    if is_loopback_host(&configured_host) && is_loopback_host(&candidate_bare) {
        return true;
    }
    false
}

fn fallback_from_url(configured: &url::Url) -> RequestBaseUrl {
    let trimmed = configured.as_str().trim_end_matches('/').to_owned();
    RequestBaseUrl {
        base: trimmed,
        origin: configured.origin(),
    }
}

/// Exposed for unit testing; production callers use the [`FromRequestParts`]
/// impl, which reads the Host header and global config.
#[must_use]
pub fn resolve(raw_host: Option<&str>, configured: &url::Url) -> RequestBaseUrl {
    if let Some(host) = raw_host.map(str::trim).filter(|s| !s.is_empty())
        && let Ok(resolved) = build_from_host(host, configured)
    {
        return resolved;
    }
    fallback_from_url(configured)
}

fn build_from_host(raw_host: &str, configured: &url::Url) -> Result<RequestBaseUrl, &'static str> {
    if raw_host.is_empty() || raw_host.contains('/') || raw_host.contains(' ') {
        return Err("invalid host header");
    }
    if !host_in_allowlist(raw_host, configured) {
        return Err("host not in allowlist");
    }
    let host_bare = raw_host
        .rsplit_once(':')
        .map_or(raw_host, |(h, _)| h)
        .to_ascii_lowercase();
    let scheme = if is_loopback_host(&host_bare) {
        "http"
    } else {
        configured.scheme()
    };
    let base = format!("{scheme}://{raw_host}");
    let parsed = url::Url::parse(&base).map_err(|_e| "host header did not parse as URL")?;
    Ok(RequestBaseUrl {
        base: base.trim_end_matches('/').to_owned(),
        origin: parsed.origin(),
    })
}

impl<S: Send + Sync> FromRequestParts<S> for RequestBaseUrl {
    type Rejection = (StatusCode, String);

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "async signature required by the FromRequestParts trait; this \
                  extractor resolves the base URL synchronously"
    )]
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let cfg = Config::get().map_err(|e| {
            tracing::error!(error = %e, "Failed to load config for RequestBaseUrl");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration unavailable".to_owned(),
            )
        })?;
        let configured = url::Url::parse(&cfg.api_external_url).map_err(|e| {
            tracing::error!(
                error = %e,
                api_external_url = %cfg.api_external_url,
                "api_external_url is not a valid URL — bootstrap validation should have caught this"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration invalid".to_owned(),
            )
        })?;

        let raw_host = parts
            .headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok());
        Ok(resolve(raw_host, &configured))
    }
}
