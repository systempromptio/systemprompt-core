//! Caller purpose carried on a request so the credential broker can see why a
//! credential is being asked for. It is set by a credential's issuer, never
//! by its bearer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RequestContext;

impl RequestContext {
    /// Binds the caller purpose (for example `workflow:<id>`), forwarded to
    /// the credential broker as `x-systemprompt-caller`. Only an issuer-bound
    /// source may call this: an API key's `purpose`, a JWT's `purpose` claim,
    /// or in-process code. Never set it from a request header.
    pub fn with_caller_purpose(mut self, purpose: String) -> Self {
        self.execution.caller_purpose = Some(purpose);
        self
    }

    pub fn caller_purpose(&self) -> Option<&str> {
        self.execution.caller_purpose.as_deref()
    }
}
