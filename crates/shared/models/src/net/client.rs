//! Connect-time SSRF enforcement for outbound HTTP clients.
//!
//! [`validate_outbound_url`](super::validate_outbound_url) can only see what a
//! URL string says. A hostname is not an address, so parse-time validation
//! cannot decide whether `metadata.example.com` is a public host or an `A`
//! record pointing at `169.254.169.254`. This module moves the decision to the
//! point where the address is actually known.
//!
//! [`GuardedResolver`] is installed as the client's DNS resolver, so every
//! name reqwest resolves — for the initial request and for every redirect hop,
//! since each hop connects afresh — is filtered through
//! [`is_blocked_ip`] before a socket is opened. The
//! redirect policy re-runs the parse-time guard on each hop as well, which
//! catches a downgrade to a non-HTTPS scheme or a literal blocked address that
//! never reaches the resolver.
//!
//! Literal-IP URLs bypass DNS entirely; those are covered by the parse-time
//! guard, which every caller runs before handing the URL to the client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use thiserror::Error;

use super::{
    HTTP_CONNECT_TIMEOUT, is_blocked_ip, trusted_http_hosts_from_env,
    validate_outbound_url_with_trust,
};

const LOOPBACK_HOST: &str = "localhost";

type ConnectError = Box<dyn std::error::Error + Send + Sync>;

fn boxed(error: GuardedConnectError) -> ConnectError {
    Box::new(error)
}

/// Why a guarded client refused to open a connection.
#[derive(Debug, Error)]
pub enum GuardedConnectError {
    #[error("cannot resolve {0}")]
    Unresolvable(String),
    #[error("host {host} resolves to blocked address {addr}")]
    BlockedAddress {
        host: String,
        addr: std::net::IpAddr,
    },
    #[error("redirect to {url} refused: {reason}")]
    RedirectRefused { url: String, reason: String },
    #[error("more than {0} redirects")]
    TooManyRedirects(usize),
}

/// Bounds applied to every request a guarded client makes.
///
/// `trusted_hosts` mirrors the `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS` allowance the
/// parse-time guard honours: a host named there keeps working even when it
/// resolves inside a blocked range, which is what lets an operator point the
/// platform at an internal service on purpose. `allow_loopback` covers
/// `localhost` for local development; literal loopback addresses never reach
/// the resolver and are governed by the parse-time guard alone.
#[derive(Debug, Clone)]
pub struct GuardedClientConfig {
    pub trusted_hosts: Vec<String>,
    pub allow_loopback: bool,
    pub max_redirects: usize,
    pub timeout: Option<Duration>,
    pub connect_timeout: Duration,
    pub user_agent: Option<String>,
}

impl Default for GuardedClientConfig {
    fn default() -> Self {
        Self {
            trusted_hosts: trusted_http_hosts_from_env(),
            allow_loopback: true,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            timeout: Some(super::HTTP_DEFAULT_TIMEOUT),
            connect_timeout: HTTP_CONNECT_TIMEOUT,
            user_agent: None,
        }
    }
}

pub const DEFAULT_MAX_REDIRECTS: usize = 3;

impl GuardedClientConfig {
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    #[must_use]
    pub const fn with_max_redirects(mut self, max_redirects: usize) -> Self {
        self.max_redirects = max_redirects;
        self
    }

    #[must_use]
    pub fn with_trusted_hosts(mut self, trusted_hosts: Vec<String>) -> Self {
        self.trusted_hosts = trusted_hosts;
        self
    }

    #[must_use]
    pub const fn deny_loopback(mut self) -> Self {
        self.allow_loopback = false;
        self
    }

    fn allowed_hosts(&self) -> Vec<String> {
        let mut allowed: Vec<String> = self
            .trusted_hosts
            .iter()
            .map(|h| h.trim().to_ascii_lowercase())
            .filter(|h| !h.is_empty())
            .collect();
        if self.allow_loopback {
            allowed.push(LOOPBACK_HOST.to_owned());
        }
        allowed
    }
}

/// DNS resolver that refuses to hand reqwest an address the SSRF block list
/// covers.
///
/// `allowed` holds already-lowercased hostnames exempted from the block list.
#[derive(Debug, Clone)]
pub struct GuardedResolver {
    allowed: Vec<String>,
}

impl GuardedResolver {
    #[must_use]
    pub fn new(allowed: Vec<String>) -> Self {
        Self {
            allowed: allowed
                .into_iter()
                .map(|h| h.to_ascii_lowercase())
                .collect(),
        }
    }
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_ascii_lowercase();
        let exempt = self.allowed.iter().any(|h| h == &host);
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|e| {
                    tracing::warn!(host = %host, error = %e, "Outbound DNS resolution failed");
                    boxed(GuardedConnectError::Unresolvable(host.clone()))
                })?
                .collect();
            if addrs.is_empty() {
                return Err(boxed(GuardedConnectError::Unresolvable(host)));
            }
            if !exempt && let Some(blocked) = addrs.iter().find(|a| is_blocked_ip(a.ip())) {
                tracing::warn!(
                    host = %host,
                    addr = %blocked.ip(),
                    "Refused outbound connection to blocked address"
                );
                return Err(boxed(GuardedConnectError::BlockedAddress {
                    host,
                    addr: blocked.ip(),
                }));
            }
            let resolved: Addrs = Box::new(addrs.into_iter());
            Ok(resolved)
        })
    }
}

pub fn guarded_client_builder(config: &GuardedClientConfig) -> reqwest::ClientBuilder {
    let trusted = config.allowed_hosts();
    let resolver = GuardedResolver::new(trusted.clone());
    // Why: with following disabled the 3xx must come back as the response so
    // the caller can see its status; a custom policy that errors on the first
    // hop would turn it into a transport failure instead.
    let policy = if config.max_redirects == 0 {
        reqwest::redirect::Policy::none()
    } else {
        guarded_redirect_policy(trusted, config.max_redirects)
    };

    let mut builder = reqwest::Client::builder()
        .dns_resolver(std::sync::Arc::new(resolver))
        .redirect(policy)
        .connect_timeout(config.connect_timeout);
    if let Some(timeout) = config.timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(user_agent) = &config.user_agent {
        builder = builder.user_agent(user_agent.clone());
    }
    builder
}

fn guarded_redirect_policy(
    trusted: Vec<String>,
    max_redirects: usize,
) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= max_redirects {
            return attempt.error(GuardedConnectError::TooManyRedirects(max_redirects));
        }
        match validate_outbound_url_with_trust(attempt.url().as_str(), &trusted) {
            Ok(_) => attempt.follow(),
            Err(e) => {
                let refused = GuardedConnectError::RedirectRefused {
                    url: attempt.url().to_string(),
                    reason: e.to_string(),
                };
                tracing::warn!(error = %refused, "Refused outbound redirect");
                attempt.error(refused)
            },
        }
    })
}

pub fn guarded_client(config: &GuardedClientConfig) -> reqwest::Result<reqwest::Client> {
    guarded_client_builder(config).build()
}
