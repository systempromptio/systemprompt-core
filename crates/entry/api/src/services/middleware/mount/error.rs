//! The typed refusals a route declaration can meet at mount.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{LoaderError, ProtectionLayer};

use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{Allowance, BaseRef};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::MissingDependency;

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
        "route group at '{base_path}' pairs a handler-verified policy with a context middleware, \
         or the handler-resolved context with a gated policy"
    )]
    IncoherentHandlerVerification { base_path: &'static str },

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

    #[error("privileged shape at {base:?} not listed in PRIVILEGED_SHAPES")]
    UnlistedPrivilege { base: BaseRef, allowance: Allowance },

    #[error("privilege at {base:?} held by {owner}, listed for {expected:?}")]
    PrivilegeOwnerMismatch {
        base: BaseRef,
        owner: ExtensionId,
        expected: CoreId,
    },

    #[error("route set at {0:?} is not a path route set")]
    RouteSetKind(BaseRef),

    #[error("route under reserved prefix {0}")]
    ReservedPrefix(&'static str),

    #[error("gateway route shadows built-in {0}")]
    GatewayShadowsBuiltin(&'static str),

    #[error("route {method} {path} declared twice")]
    RouteConflict { method: http::Method, path: String },

    #[error("dependency resolution failed")]
    Resolve(#[from] MissingDependency),
}
