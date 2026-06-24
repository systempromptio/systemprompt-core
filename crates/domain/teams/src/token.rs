//! Outbound Bot Connector token acquisition.
//!
//! Replying to a Teams activity requires an `OAuth2` client-credentials access
//! token minted against the Bot Framework login authority. Tokens are cached
//! in-process and refreshed shortly before expiry, so a burst of replies shares
//! a single token. The login URL passes the shared SSRF guard before any
//! request is made.

use std::sync::RwLock;

use serde::Deserialize;
use systemprompt_models::net::validate_outbound_url;

use crate::error::{TeamsError, TeamsResult};

const LOGIN_URL: &str = "https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token";
const SCOPE: &str = "https://api.botframework.com/.default";

/// Refresh this many seconds before the reported expiry to absorb clock skew
/// and request latency.
const REFRESH_SKEW_SECS: i64 = 60;

#[derive(Debug, Clone)]
pub struct CachedToken {
    access_token: String,
    expires_at_unix: i64,
}

impl CachedToken {
    /// Build a cache entry from a token response, applying the refresh skew so
    /// the token is treated as expired `REFRESH_SKEW_SECS` before its reported
    /// lifetime ends.
    #[must_use]
    pub const fn new(access_token: String, now_unix: i64, expires_in: i64) -> Self {
        Self {
            access_token,
            expires_at_unix: now_unix + expires_in - REFRESH_SKEW_SECS,
        }
    }

    /// Whether the cached token is still usable at `now_unix`. The expiry is
    /// exclusive: a token whose skew-adjusted expiry equals `now_unix` is
    /// already stale.
    #[must_use]
    pub const fn is_valid(&self, now_unix: i64) -> bool {
        self.expires_at_unix > now_unix
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

/// Mints and caches Bot Connector access tokens for one app registration.
#[derive(Debug)]
pub struct TokenProvider {
    http: reqwest::Client,
    app_id: String,
    app_password: String,
    token_url: String,
    cache: RwLock<Option<CachedToken>>,
}

impl TokenProvider {
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
    ) -> Self {
        Self {
            http,
            app_id: app_id.into(),
            app_password: app_password.into(),
            token_url: LOGIN_URL.to_owned(),
            cache: RwLock::new(None),
        }
    }

    /// Build a provider whose token endpoint is overridden, so a test can
    /// intercept the client-credentials request with a loopback mock server.
    #[cfg(feature = "test")]
    #[must_use]
    pub fn with_token_url(
        http: reqwest::Client,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            app_id: app_id.into(),
            app_password: app_password.into(),
            token_url: token_url.into(),
            cache: RwLock::new(None),
        }
    }

    /// Return a valid access token, minting a fresh one when the cache is empty
    /// or within the refresh window. `now_unix` is injected so expiry handling
    /// is testable.
    pub async fn token(&self, now_unix: i64) -> TeamsResult<String> {
        if let Some(cached) = self.cached_valid(now_unix) {
            return Ok(cached);
        }
        let fresh = self.fetch(now_unix).await?;
        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(fresh.clone());
        }
        Ok(fresh.access_token)
    }

    fn cached_valid(&self, now_unix: i64) -> Option<String> {
        let guard = self.cache.read().ok()?;
        let token = guard
            .as_ref()
            .filter(|cached| cached.is_valid(now_unix))
            .map(|cached| cached.access_token.clone());
        drop(guard);
        token
    }

    async fn fetch(&self, now_unix: i64) -> TeamsResult<CachedToken> {
        validate_outbound_url(&self.token_url)
            .map_err(|e| TeamsError::OutboundUrl(e.to_string()))?;
        let resp = self
            .http
            .post(&self.token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.app_id.as_str()),
                ("client_secret", self.app_password.as_str()),
                ("scope", SCOPE),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(TeamsError::Outbound(format!(
                "token endpoint returned {status}: {body}"
            )));
        }
        let parsed: TokenResponse = resp.json().await?;
        Ok(CachedToken::new(
            parsed.access_token,
            now_unix,
            parsed.expires_in,
        ))
    }
}
