//! Which credential authenticated a request: a first-party browser session
//! cookie, or a bearer token. It is set by the auth middleware from where it
//! found the token, never from a header the caller names, and decides
//! whether document calls run as the person or as an agent.
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
}
