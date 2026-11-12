//! Bounds and destination policy for a guarded client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_models::net::{
    HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT, LoopbackPolicy, OutboundUrlError, ProxyEnvironment,
    trusted_http_hosts_from_env, validate_outbound_url_with_trust,
};

const LOOPBACK_HOST: &str = "localhost";

pub const DEFAULT_MAX_REDIRECTS: usize = 3;

/// Bounds applied to every request a guarded client makes.
///
/// Built through [`Self::for_configured`] for operator-configured
/// destinations: loopback allowed, the `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS`
/// allowance honoured. Destinations a user or third party chose use
/// [`UntrustedClientConfig`] from [`Self::for_untrusted`] instead, which only
/// [`GuardedClient::untrusted`](super::GuardedClient::untrusted) accepts.
#[derive(Debug, Clone)]
pub struct GuardedClientConfig {
    trusted_hosts: Vec<String>,
    loopback: LoopbackPolicy,
    max_redirects: usize,
    timeout: Option<Duration>,
    connect_timeout: Duration,
    user_agent: Option<String>,
    proxy: Option<url::Url>,
    env_proxy_hosts: Vec<String>,
}

impl GuardedClientConfig {
    fn with_policy(trusted_hosts: Vec<String>, loopback: LoopbackPolicy) -> Self {
        Self {
            trusted_hosts: normalise(trusted_hosts),
            loopback,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            timeout: Some(HTTP_DEFAULT_TIMEOUT),
            connect_timeout: HTTP_CONNECT_TIMEOUT,
            user_agent: None,
            proxy: None,
            env_proxy_hosts: ProxyEnvironment::from_env().proxy_hosts().to_vec(),
        }
    }

    #[must_use]
    pub fn for_configured() -> Self {
        Self::with_policy(trusted_http_hosts_from_env(), LoopbackPolicy::Allow)
    }

    #[must_use]
    pub fn for_untrusted() -> UntrustedClientConfig {
        UntrustedClientConfig(Self::with_policy(Vec::new(), LoopbackPolicy::Deny))
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[must_use]
    pub const fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    #[must_use]
    pub const fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
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
        self.trusted_hosts = normalise(trusted_hosts);
        self
    }

    #[must_use]
    pub fn with_proxy(mut self, proxy: url::Url) -> Self {
        self.proxy = Some(proxy);
        self
    }

    #[must_use]
    pub const fn proxy(&self) -> Option<&url::Url> {
        self.proxy.as_ref()
    }

    #[must_use]
    pub const fn loopback(&self) -> LoopbackPolicy {
        self.loopback
    }

    #[must_use]
    pub fn trusted_hosts(&self) -> &[String] {
        &self.trusted_hosts
    }

    #[must_use]
    pub const fn max_redirects(&self) -> usize {
        self.max_redirects
    }

    #[must_use]
    pub const fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    #[must_use]
    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    pub(super) fn check_url(&self, url: &str) -> Result<url::Url, OutboundUrlError> {
        let parsed = validate_outbound_url_with_trust(url, &self.trusted_hosts, self.loopback)?;
        refuse_proxy_target(&parsed, self.loopback, &self.proxy_hosts())?;
        Ok(parsed)
    }

    pub(super) fn proxy_hosts(&self) -> Vec<String> {
        let mut hosts = self.env_proxy_hosts.clone();
        hosts.extend(
            self.proxy
                .as_ref()
                .and_then(url::Url::host_str)
                .map(str::to_ascii_lowercase),
        );
        hosts
    }

    pub(super) fn resolver_exemptions(&self) -> Vec<String> {
        let mut allowed = self.trusted_hosts.clone();
        if self.loopback == LoopbackPolicy::Allow {
            allowed.push(LOOPBACK_HOST.to_owned());
        }
        allowed.extend(self.proxy_hosts());
        allowed
    }
}

pub(super) fn refuse_proxy_target(
    target: &url::Url,
    loopback: LoopbackPolicy,
    proxy_hosts: &[String],
) -> Result<(), OutboundUrlError> {
    let host = target.host_str().unwrap_or_default().to_ascii_lowercase();
    if loopback == LoopbackPolicy::Deny && proxy_hosts.iter().any(|p| p == &host) {
        return Err(OutboundUrlError::ProxyHost(host));
    }
    Ok(())
}

fn normalise(hosts: Vec<String>) -> Vec<String> {
    hosts
        .into_iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect()
}

/// Bounds for a client whose destinations a user or third party chose:
/// loopback denied, no trusted hosts unless named, and the forward proxy
/// host never accepted as a target.
///
/// It cannot be turned into a plain [`GuardedClientConfig`], so the only
/// client it builds is a [`GuardedClient`](super::GuardedClient), whose every
/// request validates its URL first.
#[derive(Debug, Clone)]
pub struct UntrustedClientConfig(GuardedClientConfig);

impl UntrustedClientConfig {
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self(self.0.with_timeout(timeout))
    }

    #[must_use]
    pub fn with_user_agent(self, user_agent: impl Into<String>) -> Self {
        Self(self.0.with_user_agent(user_agent))
    }

    #[must_use]
    pub fn with_max_redirects(self, max_redirects: usize) -> Self {
        Self(self.0.with_max_redirects(max_redirects))
    }

    #[must_use]
    pub fn with_trusted_hosts(self, trusted_hosts: Vec<String>) -> Self {
        Self(self.0.with_trusted_hosts(trusted_hosts))
    }

    #[must_use]
    pub fn with_proxy(self, proxy: url::Url) -> Self {
        Self(self.0.with_proxy(proxy))
    }

    #[must_use]
    pub fn trusted_hosts(&self) -> &[String] {
        self.0.trusted_hosts()
    }

    #[must_use]
    pub const fn loopback(&self) -> LoopbackPolicy {
        self.0.loopback()
    }

    pub(super) const fn inner(&self) -> &GuardedClientConfig {
        &self.0
    }
}
