//! The typed refusals a route declaration can meet at mount.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{LoaderError, ProtectionLayer};

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("route group at '{base_path}' declares a policy that admits no caller type")]
    NoAdmittedCaller { base_path: &'static str },

    #[error(
        "route group at '{base_path}' declares a bearer window but the server has no bearer \
         extractor"
    )]
    BearerWindowUnavailable { base_path: &'static str },

    #[error(
        "route group at '{base_path}' declares the {layer:?} protection layer but the server has \
         no guard for it"
    )]
    GuardUnavailable {
        base_path: &'static str,
        layer: ProtectionLayer,
    },

    #[error(
        "route group at '{base_path}' pairs the public context with a policy that does not admit \
         anonymous callers, so no request can reach it"
    )]
    IncoherentContext { base_path: &'static str },

    #[error(
        "extension route group at '{base_path}' declares a {declared}-byte body limit above the \
         {ceiling}-byte server default"
    )]
    ExtensionBodyLimitTooLarge {
        base_path: &'static str,
        declared: usize,
        ceiling: usize,
    },

    #[error(
        "route group at '{base_path}' pairs a handler-verified policy with a context middleware, \
         or the handler-resolved context with a gated policy"
    )]
    IncoherentHandlerVerification { base_path: &'static str },

    #[error(
        "extension route group at '{base_path}' declares a handler-verified policy or the \
         handler-resolved context, which only core route groups may declare"
    )]
    ExtensionHandlerVerified { base_path: &'static str },

    #[error(
        "core route group at '{base_path}' declares a {declared}-byte body limit above the \
         2 MiB server default that no core body exemption names"
    )]
    UnlistedBodyExemption {
        base_path: &'static str,
        declared: usize,
    },

    #[error("route group at '{base_path}' could not build its rate limit")]
    RateLimit {
        base_path: &'static str,
        #[source]
        source: Box<LoaderError>,
    },
}
