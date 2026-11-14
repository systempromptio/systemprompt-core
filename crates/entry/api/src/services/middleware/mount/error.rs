//! The typed refusals a route declaration can meet at mount.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{LoaderError, ProtectionLayer};

use super::CoreShapeField;

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
        "extension '{extension}' declares a route group at the reserved base path '{base_path}', \
         which only a core route group listed in the core allowance table may declare"
    )]
    ExtensionReservedPath {
        extension: &'static str,
        base_path: &'static str,
    },

    #[error(
        "extension '{extension}' declares the core route group at '{base_path}' with a protection \
         shape whose {field:?} differs from the one the core declaration table lists for it"
    )]
    CoreDeclarationMismatch {
        extension: &'static str,
        base_path: &'static str,
        field: CoreShapeField,
    },

    #[error(
        "extension '{extension}' registers a gateway route group at '{path}' that no core \
         declaration lists"
    )]
    UnlistedGatewayRoute {
        extension: &'static str,
        path: &'static str,
    },

    #[error("gateway route group at '{path}' overlaps the gateway route group at '{other}'")]
    GatewayRouteOverlap {
        path: &'static str,
        other: &'static str,
    },

    #[error(
        "route group at '{base_path}' declares a {declared}-byte body limit above the 2 MiB \
         server default that no core allowance names for that base path"
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
