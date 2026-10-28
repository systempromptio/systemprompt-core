//! Which credential authenticated a request. `browser_session` is set only
//! when the token arrived as the session cookie *and* carries the
//! `browser_session` claim the first-party web login mints; `cookie_transport`
//! records the cookie alone. The auth middleware sets both, never from a
//! header the caller names; they decide whether document calls run as the
//! person or as an agent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RequestContext;

impl RequestContext {
    #[must_use]
    pub const fn with_browser_session(mut self) -> Self {
        self.auth.browser_session = true;
        self
    }

    #[must_use]
    pub const fn is_browser_session(&self) -> bool {
        self.auth.browser_session
    }

    #[must_use]
    pub const fn with_cookie_transport(mut self) -> Self {
        self.auth.cookie_transport = true;
        self
    }

    /// The token arrived as the session cookie. Without
    /// [`Self::is_browser_session`] it is a bearer-class token presented as a
    /// cookie.
    #[must_use]
    pub const fn is_cookie_transport(&self) -> bool {
        self.auth.cookie_transport
    }
}
