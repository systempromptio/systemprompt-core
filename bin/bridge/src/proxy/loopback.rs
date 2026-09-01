//! Where the loopback proxy is reachable from this process: its port and the
//! shared secret a host presents to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::LoopbackSecret;
use crate::proxy::secret;

/// The loopback proxy as seen by everything that writes a host profile, an
/// MCP fragment or a hooks file.
///
/// A value, not a lookup: the composition root resolves the port once (the
/// port this process bound, the port a sibling bridge answers on, or the
/// recorded port) and hands the same answer to every writer, so a profile and
/// the hooks that ride with it can never disagree about where the proxy is.
#[derive(Clone)]
pub struct LoopbackEndpoint {
    port: u16,
    secret: Option<LoopbackSecret>,
}

impl std::fmt::Debug for LoopbackEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopbackEndpoint")
            .field("port", &self.port)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl LoopbackEndpoint {
    #[must_use]
    pub const fn new(port: u16, secret: Option<LoopbackSecret>) -> Self {
        Self { port, secret }
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    #[must_use]
    pub fn mcp_url(&self, slug: &str) -> String {
        format!("{}/mcp/{slug}", self.origin())
    }

    // Why: a process that did not mint the secret (an `install --apply` run, a
    // GUI that lost the port race) reads it from disk at the moment of use, so
    // a secret minted by the serving bridge after this process started is still
    // found.
    pub fn secret(&self) -> std::io::Result<LoopbackSecret> {
        self.secret
            .as_ref()
            .map_or_else(secret::for_profile, |s| Ok(s.clone()))
    }

    // Why: a fragment writer (`sync`, `install --apply`) may legitimately run
    // before the proxy has ever started; it mints the secret the proxy will
    // later load, where a *profile* must only ever carry one that exists.
    pub fn bearer(&self) -> std::io::Result<String> {
        self.secret
            .as_ref()
            .map_or_else(secret::proxy_init, |s| Ok(s.clone()))
            .map(|s| format!("Bearer {}", s.as_str()))
    }

    #[must_use]
    pub fn secret_fingerprint(&self) -> Option<String> {
        self.secret().ok().map(|s| secret::fingerprint(s.as_str()))
    }
}
