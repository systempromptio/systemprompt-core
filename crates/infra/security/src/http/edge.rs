//! The request-edge facts the shared HTTP extractors read.
//!
//! [`EdgeConfigView`] is the one configuration value the API installs on
//! every router: the public URL, the advertised URL and the trusted proxies.
//! [`ClientIp`](super::ClientIp), [`RequestBaseUrl`](super::RequestBaseUrl)
//! and the document routes' same-origin check read it; nothing else about the
//! runtime configuration reaches a route through a request extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use ipnet::IpNet;
use systemprompt_manifest::Config;

/// The public URL, advertised URL and trusted proxies, as a request extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeConfigView {
    pub api_external_url: String,
    pub advertise_url: Option<String>,
    pub trusted_proxies: Vec<IpNet>,
}

impl EdgeConfigView {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            api_external_url: config.api_external_url.clone(),
            advertise_url: config.advertise_url.clone(),
            trusted_proxies: config.trusted_proxies.clone(),
        }
    }
}
