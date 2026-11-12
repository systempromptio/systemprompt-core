//! SSRF guard for server-side fetches of caller-supplied image URLs.
//!
//! The URL is chosen by whoever sent the inference request, not by an
//! operator, so the client is a [`GuardedClient`] built from the untrusted
//! policy: every request validates its URL with loopback denied, the
//! connect-time resolver checks every address a name resolves to on the first
//! request and every redirect hop, and no environment proxy is used.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_client::{GuardedClient, GuardedClientConfig};

use super::{ImageFetchFault, ImageFetchPolicy};

pub(super) fn client(policy: &ImageFetchPolicy) -> Result<GuardedClient, ImageFetchFault> {
    let config = GuardedClientConfig::for_untrusted()
        .with_trusted_hosts(policy.trusted_hosts.clone())
        .with_max_redirects(usize::from(policy.max_redirects))
        .with_timeout(policy.timeout);
    GuardedClient::untrusted(&config).map_err(ImageFetchFault::Client)
}
