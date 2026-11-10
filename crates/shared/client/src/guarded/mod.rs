//! Connect-time SSRF enforcement for outbound HTTP clients.
//!
//! [`validate_outbound_url_with_trust`] can only see what a URL string says. A
//! hostname is not an address, so parse-time validation cannot decide whether
//! `metadata.example.com` is a public host or an `A` record pointing at
//! `169.254.169.254`. This module moves the decision to the point where the
//! address is actually known.
//!
//! [`GuardedResolver`] is installed as the client's DNS resolver, so every
//! name reqwest resolves — for the initial request and for every redirect hop,
//! since each hop connects afresh — is filtered through
//! [`is_blocked_ip`](systemprompt_models::net::is_blocked_ip)
//! before a socket is opened, and reqwest connects only to the addresses the
//! resolver returned, so the vetted address is the one dialled. The redirect
//! policy re-runs the parse-time guard on each hop with the client's
//! [`LoopbackPolicy`]; a hop may land on loopback only when the first URL of
//! the chain was itself loopback and the policy allows it.
//!
//! Literal-IP URLs bypass DNS entirely. A redirect hop to one is covered by the
//! redirect policy; the initial URL is covered by [`GuardedClient`], whose
//! only request path validates it. A client for untrusted destinations can
//! only be a [`GuardedClient`] and never uses an environment proxy, which
//! would otherwise resolve the target where the guard cannot see it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod config;
mod resolver;

pub use client::GuardedClient;
pub use config::{DEFAULT_MAX_REDIRECTS, GuardedClientConfig, UntrustedClientConfig};
pub use resolver::GuardedResolver;

use thiserror::Error;

use systemprompt_models::net::{
    LoopbackPolicy, OutboundUrlError, is_loopback_host, validate_outbound_url_with_trust,
};

/// Why a guarded client refused to open a connection.
#[derive(Debug, Error)]
pub enum GuardedConnectError {
    #[error("cannot resolve {host}")]
    Unresolvable {
        host: String,
        #[source]
        source: Option<std::io::Error>,
    },
    #[error("host {host} resolves to blocked address {addr}")]
    BlockedAddress {
        host: String,
        addr: std::net::IpAddr,
    },
    #[error("redirect to {url} refused: {source}")]
    RedirectRefused {
        url: String,
        #[source]
        source: OutboundUrlError,
    },
    #[error("more than {0} redirects")]
    TooManyRedirects(usize),
}

pub fn guarded_client_builder(config: &GuardedClientConfig) -> reqwest::ClientBuilder {
    let resolver = GuardedResolver::new(config.resolver_exemptions());
    // Why: with following disabled the 3xx must come back as the response so
    // the caller can see its status; a custom policy that errors on the first
    // hop would turn it into a transport failure instead.
    let policy = if config.max_redirects() == 0 {
        reqwest::redirect::Policy::none()
    } else {
        guarded_redirect_policy(
            config.trusted_hosts().to_vec(),
            config.loopback(),
            config.max_redirects(),
        )
    };

    let mut builder = reqwest::Client::builder()
        .dns_resolver(std::sync::Arc::new(resolver))
        .redirect(policy)
        .connect_timeout(config.connect_timeout());
    if let Some(timeout) = config.timeout() {
        builder = builder.timeout(timeout);
    }
    if let Some(user_agent) = config.user_agent() {
        builder = builder.user_agent(user_agent.to_owned());
    }
    builder
}

fn hop_policy(first: Option<&url::Url>, loopback: LoopbackPolicy) -> LoopbackPolicy {
    let first_is_loopback = first
        .and_then(url::Url::host)
        .is_some_and(|host| is_loopback_host(&host));
    if loopback == LoopbackPolicy::Allow && first_is_loopback {
        LoopbackPolicy::Allow
    } else {
        LoopbackPolicy::Deny
    }
}

fn guarded_redirect_policy(
    trusted: Vec<String>,
    loopback: LoopbackPolicy,
    max_redirects: usize,
) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        // Why: reqwest pushes the URL being left onto `previous` before asking,
        // so the first hop already sees one entry; `>` follows exactly
        // `max_redirects` hops, matching `Policy::limited`.
        if attempt.previous().len() > max_redirects {
            return attempt.error(GuardedConnectError::TooManyRedirects(max_redirects));
        }
        let policy = hop_policy(attempt.previous().first(), loopback);
        match validate_outbound_url_with_trust(attempt.url().as_str(), &trusted, policy) {
            Ok(_) => attempt.follow(),
            Err(source) => {
                let refused = GuardedConnectError::RedirectRefused {
                    url: attempt.url().to_string(),
                    source,
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
