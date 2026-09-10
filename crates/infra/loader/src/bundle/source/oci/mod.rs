//! OCI distribution transport for services bundles.
//!
//! Only the parts of the distribution spec a bundle needs are implemented:
//! a manifest GET, the Bearer challenge dance, and a single blob pull whose
//! `mediaType` is
//! [`BUNDLE_MEDIA_TYPE`](systemprompt_models::services::bundle::BUNDLE_MEDIA_TYPE).
//! A manifest carrying zero or several
//! such layers is refused rather than guessed at, because picking one would
//! make which bytes an instance runs depend on registry ordering.
//!
//! The registry scheme is `https` unless the host is loopback or listed in
//! the trusted-host escape hatch, and every constructed URL goes through the
//! shared SSRF guard.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod pull;
pub mod push;

use std::path::Path;
use std::str::FromStr;

use systemprompt_models::net::{trusted_http_hosts_from_env, validate_outbound_url_with_trust};
use systemprompt_models::profile::OciReference;

use super::{BundleFetcher, FetchedBundle, MAX_BUNDLE_BYTES, RemoteRef};
use crate::bundle::error::{BundleError, BundleResult};

pub use push::push_bundle;

#[derive(Debug)]
pub struct RegistryClient {
    pub client: reqwest::Client,
    pub name: String,
    pub reference: OciReference,
    pub secret: Option<String>,
}

impl RegistryClient {
    pub fn new(
        name: &str,
        reference: &str,
        secret: Option<String>,
        client: reqwest::Client,
    ) -> BundleResult<Self> {
        let reference = OciReference::from_str(reference)
            .map_err(|e| BundleError::policy(format!("source {name}: {e}")))?;
        Ok(Self {
            client,
            name: name.to_owned(),
            reference,
            secret,
        })
    }

    pub fn url(&self, path: &str) -> BundleResult<url::Url> {
        let host = &self.reference.registry;
        let trusted = trusted_http_hosts_from_env();
        let bare = host.split(':').next().unwrap_or(host);
        let plain = bare == "localhost"
            || bare == "127.0.0.1"
            || trusted.iter().any(|t| t.eq_ignore_ascii_case(bare));
        let scheme = if plain { "http" } else { "https" };
        let raw = format!("{scheme}://{host}/v2/{}{path}", self.reference.repository);
        validate_outbound_url_with_trust(&raw, &trusted)
            .map_err(|e| BundleError::fetch(&self.name, e))
    }

    #[must_use]
    pub fn manifest_ref(&self) -> String {
        self.reference.digest.clone().unwrap_or_else(|| {
            self.reference
                .tag
                .clone()
                .unwrap_or_else(|| "latest".to_owned())
        })
    }

    pub async fn send(
        &self,
        build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder + Send,
    ) -> BundleResult<reqwest::Response> {
        let first = auth::apply_credential(build(&self.client), self.secret.as_deref())
            .send()
            .await
            .map_err(|e| BundleError::fetch(&self.name, e))?;

        if first.status() != reqwest::StatusCode::UNAUTHORIZED {
            return Ok(first);
        }

        let header = first
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let challenge = auth::parse_challenge(&header).ok_or_else(|| BundleError::Auth {
            source_name: self.name.clone(),
        })?;
        let token =
            auth::fetch_token(&self.client, &challenge, self.secret.as_deref(), &self.name).await?;

        let retried = build(&self.client)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| BundleError::fetch(&self.name, e))?;
        if retried.status() == reqwest::StatusCode::UNAUTHORIZED
            || retried.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(BundleError::Auth {
                source_name: self.name.clone(),
            });
        }
        Ok(retried)
    }
}

#[derive(Debug)]
pub struct OciFetcher {
    registry: RegistryClient,
    max_bytes: u64,
}

impl OciFetcher {
    pub fn new(
        name: &str,
        reference: &str,
        secret: Option<String>,
        client: reqwest::Client,
    ) -> BundleResult<Self> {
        Ok(Self {
            registry: RegistryClient::new(name, reference, secret, client)?,
            max_bytes: MAX_BUNDLE_BYTES,
        })
    }

    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }
}

impl BundleFetcher for OciFetcher {
    async fn head(&self) -> BundleResult<RemoteRef> {
        let (digest, _manifest) = pull::get_manifest(&self.registry).await?;
        Ok(RemoteRef { digest })
    }

    async fn fetch(&self, into: &Path) -> BundleResult<FetchedBundle> {
        let (_digest, manifest) = pull::get_manifest(&self.registry).await?;
        pull::pull_bundle_layer(&self.registry, &manifest, into, self.max_bytes).await
    }
}
