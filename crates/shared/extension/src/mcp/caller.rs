//! The caller a tool execution is recorded for.
//!
//! A [`VerifiedCaller`] is the request context the toolset host restored from
//! a verified platform hop token: the kernel's proxy signed the caller's
//! identity into the hop, and the host's admission (`admit_request` in
//! `systemprompt-mcp-host`) checked it before inserting the caller into the
//! request. [`super::execute`] and [`super::ToolExecution::begin`] take one,
//! so an execution is recorded only for the identity a hop vouched for and
//! never for a context a toolset or member command built itself. Nothing
//! outside the host's admission builds one (`lint-hidden-constructors`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::RequestContext;

/// The hop-verified caller of one MCP request.
#[derive(Debug, Clone)]
pub struct VerifiedCaller {
    context: RequestContext,
}

impl VerifiedCaller {
    #[must_use]
    pub const fn context(&self) -> &RequestContext {
        &self.context
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn __from_hop(context: RequestContext) -> Self {
        Self { context }
    }
}
