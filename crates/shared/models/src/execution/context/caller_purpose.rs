//! Caller purpose carried on a request so the credential broker can see why a
//! credential is being asked for. It is set by a credential's issuer, never
//! by its bearer.
//!
//! Only an issuer-bound source may bind the caller purpose: an API key's
//! `purpose`, a JWT's `purpose` claim, or in-process code. It is never set from
//! a request header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RequestContext;

impl RequestContext {
    pub fn with_caller_purpose(mut self, purpose: String) -> Self {
        self.execution.caller_purpose = Some(purpose);
        self
    }

    pub fn caller_purpose(&self) -> Option<&str> {
        self.execution.caller_purpose.as_deref()
    }
}
