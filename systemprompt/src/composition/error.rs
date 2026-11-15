//! The refusals a composition's declaration and admission can raise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{ContributionError, ExtensionConfigError, SlotError};
use systemprompt_identifiers::ExtensionId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_traits::authz::SubjectDimension;

#[cfg(feature = "cli")]
use systemprompt_extension::cli::KernelGrant;
#[cfg(feature = "api")]
use systemprompt_extension::router::{Allowance, BasePath, BaseRef};

#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    #[error("unknown extension in profile extensions.disabled: {0}")]
    UnknownExtension(String),
    #[error("extension id is invalid: {id}")]
    InvalidId {
        id: &'static str,
        #[source]
        source: IdValidationError,
    },
    #[error("duplicate extension id {0}")]
    DuplicateId(ExtensionId),
    #[error("required extension {0} cannot be disabled or removed")]
    RequiredDisabled(ExtensionId),
    #[error("{owner} requires {ty}, which no enabled member provides")]
    Missing {
        owner: ExtensionId,
        ty: &'static str,
    },
    #[error("{owner} requires {ty}; its core provider {provider} is disabled")]
    DisabledDependency {
        owner: ExtensionId,
        ty: &'static str,
        provider: ExtensionId,
    },
    #[error("{owner} (core) requires {ty}, provided only by install member {provider}")]
    InstallSubstitution {
        owner: ExtensionId,
        ty: &'static str,
        provider: ExtensionId,
    },
    #[error("{ty} provided twice: {first} and {second}")]
    DuplicateProvider {
        ty: &'static str,
        first: ExtensionId,
        second: ExtensionId,
    },
    #[error("{owner} depends on {target} across capabilities")]
    CrossCapabilityDependency {
        owner: ExtensionId,
        target: ExtensionId,
    },
    #[cfg(feature = "api")]
    #[error("{owner}'s routes at {base:?} do not match the conferred shape")]
    ConferralShapeMismatch { owner: ExtensionId, base: BaseRef },
    #[cfg(feature = "api")]
    #[error("{owner} uses {allowance:?} at {base:?} without a conferral")]
    NotConferred {
        owner: ExtensionId,
        base: BasePath,
        allowance: Allowance,
    },
    #[error("{owner} mounts under reserved prefix {prefix}")]
    ReservedPrefix {
        owner: ExtensionId,
        prefix: &'static str,
    },
    #[error("{owner} gateway route without conferral")]
    UnlistedGatewayRoute { owner: ExtensionId },
    #[error("{owner} gateway route shadows built-in {builtin}")]
    GatewayRouteShadowsBuiltin {
        owner: ExtensionId,
        builtin: &'static str,
    },
    #[error("gateway routes overlap: {first} and {second}")]
    GatewayRouteOverlap {
        first: ExtensionId,
        second: ExtensionId,
    },
    #[cfg(feature = "api")]
    #[error("{owner} requests handle {handle:?} without conferral at {base:?}")]
    HandleNotConferred {
        owner: ExtensionId,
        handle: systemprompt_extension::HandleId,
        base: BasePath,
    },
    #[cfg(feature = "cli")]
    #[error("{owner} requests {grant:?} without conferral")]
    GrantNotConferred {
        owner: ExtensionId,
        grant: KernelGrant,
    },
    #[error("reserved CLI path {path:?} taken by {owner}")]
    ReservedCliPath {
        owner: ExtensionId,
        path: &'static [&'static str],
    },
    #[error("{leaf} attaches under {parent:?}, which nothing provides")]
    ParentMissing {
        leaf: &'static str,
        parent: &'static [&'static str],
    },
    #[error("command path {path:?} declared twice")]
    CommandConflict { path: Vec<&'static str> },
    #[error("route {method} {path} declared twice")]
    RouteConflict { method: http::Method, path: String },
    #[error("subject dimension {dimension:?} declared by {second}, already owned by {first}")]
    DimensionConflict {
        dimension: SubjectDimension,
        first: ExtensionId,
        second: ExtensionId,
    },
    #[error("{owner} declares reserved subject dimension {dimension:?}")]
    ReservedDimension {
        owner: ExtensionId,
        dimension: SubjectDimension,
    },
    #[error("secrets source kind {0} registered twice")]
    DuplicateSecretsKind(&'static str),
    #[error("secrets source kind {0} is reserved for core")]
    ReservedSecretsKind(&'static str),
    #[error("extension {owner} failed to contribute")]
    Contribution {
        owner: ExtensionId,
        #[source]
        source: ContributionError,
    },
    #[error("slot build failed")]
    Slot(#[from] SlotError),
    #[error("config for {owner} invalid")]
    Config {
        owner: ExtensionId,
        #[source]
        source: ExtensionConfigError,
    },
}

#[cfg(feature = "cli")]
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("composition refused")]
    Composition(#[from] CompositionError),
    #[error("cli run failed")]
    Cli(#[source] systemprompt_cli::CliRunError),
}
