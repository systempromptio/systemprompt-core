//! DNS resolver that applies the SSRF block list to every resolved address.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use systemprompt_models::net::is_blocked_ip;

use super::GuardedConnectError;

type ConnectError = Box<dyn std::error::Error + Send + Sync>;

fn boxed(error: GuardedConnectError) -> ConnectError {
    Box::new(error)
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
                .map_err(|source| {
                    boxed(GuardedConnectError::Unresolvable {
                        host: host.clone(),
                        source: Some(source),
                    })
                })?
                .collect();
            if addrs.is_empty() {
                return Err(boxed(GuardedConnectError::Unresolvable {
                    host,
                    source: None,
                }));
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
