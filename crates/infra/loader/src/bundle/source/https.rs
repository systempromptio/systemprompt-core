//! Plain HTTPS bundle transport.
//!
//! The URL is re-validated through the shared SSRF guard on every call rather
//! than only at profile-parse time, so a profile reloaded from a mutable
//! source cannot smuggle a link-local address past the boot path. Redirects
//! are not followed: a 3xx would let the origin move the download to a host
//! the guard never saw.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::net::{trusted_http_hosts_from_env, validate_outbound_url_with_trust};

use super::stream::stream_to_file;
use super::{BundleFetcher, FetchedBundle, MAX_BUNDLE_BYTES, RemoteRef};
use crate::bundle::error::{BundleError, BundleResult};

#[derive(Debug)]
pub struct HttpsFetcher {
    name: String,
    url: String,
    auth: Option<String>,
    client: reqwest::Client,
    max_bytes: u64,
}

impl HttpsFetcher {
    #[must_use]
    pub fn new(name: &str, url: &str, auth: Option<String>, client: reqwest::Client) -> Self {
        Self {
            name: name.to_owned(),
            url: url.to_owned(),
            auth,
            client,
            max_bytes: MAX_BUNDLE_BYTES,
        }
    }

    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    fn checked_url(&self) -> BundleResult<url::Url> {
        let trusted = trusted_http_hosts_from_env();
        validate_outbound_url_with_trust(&self.url, &trusted)
            .map_err(|e| BundleError::fetch(&self.name, e))
    }
}

impl BundleFetcher for HttpsFetcher {
    async fn head(&self) -> BundleResult<RemoteRef> {
        let url = self.checked_url()?;
        let mut request = self.client.head(url);
        if let Some(token) = self.auth.as_ref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|e| BundleError::fetch(&self.name, e))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(BundleError::Auth {
                source_name: self.name.clone(),
            });
        }
        if !response.status().is_success() {
            return Ok(RemoteRef {
                digest: String::new(),
            });
        }

        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim_matches('"').to_owned())
            .unwrap_or_default();
        Ok(RemoteRef { digest: etag })
    }

    async fn fetch(&self, into: &Path) -> BundleResult<FetchedBundle> {
        let url = self.checked_url()?;
        let mut request = self.client.get(url);
        if let Some(token) = self.auth.as_ref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|e| BundleError::fetch(&self.name, e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(BundleError::Auth {
                source_name: self.name.clone(),
            });
        }
        if status.is_redirection() {
            return Err(BundleError::fetch(
                &self.name,
                format!("redirect ({status}) is not followed"),
            ));
        }
        if !status.is_success() {
            return Err(BundleError::fetch(&self.name, format!("status {status}")));
        }

        let digest = stream_to_file(response, into, &self.name, self.max_bytes).await?;
        Ok(FetchedBundle {
            archive: into.to_path_buf(),
            digest,
        })
    }
}
