//! The caller a tool execution is recorded for.
//!
//! A [`VerifiedCaller`] is the request context the toolset host restored from
//! a platform hop token, together with that token. It is evidence, not an
//! assertion: anyone can build one with [`VerifiedCaller::presented`], and no
//! core sink takes its context on trust. The MCP member's tool execution
//! ([`super::ToolExecution::begin`]) verifies the hop token again against the
//! deployment's verification keys, this deployment's issuer and the one
//! server's audience, and refuses the execution unless the token's identity
//! vouches for the context (actor, user, session, client, user type and act
//! chain). A caller built from a forged, expired or other-server token, or a
//! context for a different user than its token, records nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::JwtToken;
use systemprompt_models::RequestContext;

/// The caller of one MCP request: the context the host built and the hop
/// token it was built from.
#[derive(Clone)]
pub struct VerifiedCaller {
    hop: JwtToken,
    context: RequestContext,
}

impl VerifiedCaller {
    #[must_use]
    pub const fn presented(hop: JwtToken, context: RequestContext) -> Self {
        Self { hop, context }
    }

    #[must_use]
    pub const fn context(&self) -> &RequestContext {
        &self.context
    }

    #[must_use]
    pub const fn hop_token(&self) -> &JwtToken {
        &self.hop
    }
}

impl std::fmt::Debug for VerifiedCaller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedCaller")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}
