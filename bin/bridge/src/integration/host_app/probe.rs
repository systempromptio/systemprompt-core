//! Context supplied to host configuration probes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::{HostId, LoopbackSecret};

/// What a host probe needs to know about the proxy to judge a profile fresh:
/// the port the proxy is on and the secret it accepts, from which the
/// per-host token a desktop policy carries is derived.
///
/// A value built by the caller from the [`crate::proxy::LoopbackEndpoint`],
/// so a probe never reaches for process state and a test can hand it any
/// port it likes.
#[derive(Debug, Clone)]
pub struct ProbeEnv {
    pub proxy_port: u16,
    pub loopback_secret: Option<LoopbackSecret>,
    pub start_menu: std::sync::Arc<crate::probe_cache::StartMenuCache>,
    pub expected_managed_servers: Option<Vec<String>>,
    pub policy_writer_ready: bool,
}

impl ProbeEnv {
    #[must_use]
    pub fn new(
        loopback: &crate::proxy::LoopbackEndpoint,
        start_menu: std::sync::Arc<crate::probe_cache::StartMenuCache>,
    ) -> Self {
        let loopback_secret = match loopback.secret() {
            Ok(secret) => Some(secret),
            Err(error) => {
                tracing::warn!(error = %error, "loopback secret is unreadable; host probes report it as unverifiable");
                None
            },
        };
        Self {
            proxy_port: loopback.port(),
            loopback_secret,
            start_menu,
            expected_managed_servers: None,
            policy_writer_ready: false,
        }
    }

    #[must_use]
    pub fn for_bridge(bridge: &crate::context::BridgeContext) -> Self {
        Self::new(
            bridge.proxy.loopback(),
            std::sync::Arc::clone(&bridge.start_menu),
        )
        .with_managed_servers(bridge.proxy.loopback(), &bridge.mcp_registry())
        .with_policy_writer()
    }

    // Why: whether a rewrite of the machine policy will raise the
    // administrator prompt depends on the writer being registered and
    // usable; the probe records it so the verb the GUI offers is right.
    #[cfg(target_os = "windows")]
    #[must_use]
    pub fn with_policy_writer(mut self) -> Self {
        self.policy_writer_ready = matches!(
            crate::install::policy_writer::status(),
            crate::install::policy_writer::WriterStatus::Ready
        );
        self
    }

    #[cfg(not(target_os = "windows"))]
    #[must_use]
    pub const fn with_policy_writer(self) -> Self {
        self
    }

    // Why: the registry is authoritative only once a sync has published it
    // (the on-disk fragment exists); before that an empty registry means
    // "not loaded", not "no servers". A catalog that cannot expand a wildcard
    // tool policy withholds the whole list, and so does this.
    #[must_use]
    pub fn with_managed_servers(
        mut self,
        loopback: &crate::proxy::LoopbackEndpoint,
        registry: &crate::mcp_registry::McpRegistry,
    ) -> Self {
        if !crate::mcp_registry::fragment_exists() {
            return self;
        }
        self.expected_managed_servers = match crate::install::mdm::policy::mcp_entries(
            loopback, registry,
        ) {
            Ok(entries) => entries.map(|list| list.into_iter().map(|e| e.name).collect()),
            Err(error) => {
                tracing::warn!(error = %error, "managed MCP servers could not be projected; the policy probe leaves them unchecked");
                None
            },
        };
        self
    }

    #[must_use]
    pub fn loopback_secret_fingerprint(&self) -> Option<String> {
        self.loopback_secret
            .as_ref()
            .map(|s| crate::proxy::secret::fingerprint(s.as_str()))
    }

    #[must_use]
    pub fn host_token_fingerprint(&self, host: &HostId) -> Option<String> {
        self.loopback_secret.as_ref().map(|s| {
            crate::proxy::secret::fingerprint(
                crate::proxy::scoped_token::host_token(s, host).as_str(),
            )
        })
    }
}
