//! The guarded client: every request validates its URL under the client's
//! own policy before it is built.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::net::{LoopbackPolicy, OutboundUrlError};

use super::config::{GuardedClientConfig, UntrustedClientConfig};
use super::guarded_client_builder;

/// HTTP client whose only request path checks the first URL against the
/// policy it was built with, so a literal-IP destination is judged even
/// though it never reaches the DNS resolver.
#[derive(Debug, Clone)]
pub struct GuardedClient {
    http: reqwest::Client,
    config: GuardedClientConfig,
}

impl GuardedClient {
    pub fn configured(config: &GuardedClientConfig) -> reqwest::Result<Self> {
        Ok(Self {
            http: guarded_client_builder(config).build()?,
            config: config.clone(),
        })
    }

    pub fn untrusted(config: &UntrustedClientConfig) -> reqwest::Result<Self> {
        let config = config.inner();
        Ok(Self {
            http: guarded_client_builder(config).no_proxy().build()?,
            config: config.clone(),
        })
    }

    #[must_use]
    pub const fn loopback(&self) -> LoopbackPolicy {
        self.config.loopback()
    }

    pub fn request(
        &self,
        method: reqwest::Method,
        url: &str,
    ) -> Result<reqwest::RequestBuilder, OutboundUrlError> {
        let url = self.config.check_url(url)?;
        Ok(self.http.request(method, url))
    }

    pub fn get(&self, url: &str) -> Result<reqwest::RequestBuilder, OutboundUrlError> {
        self.request(reqwest::Method::GET, url)
    }

    pub fn post(&self, url: &str) -> Result<reqwest::RequestBuilder, OutboundUrlError> {
        self.request(reqwest::Method::POST, url)
    }
}
