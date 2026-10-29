//! Failure modes of the gateway section on its own: route ids, route shape,
//! request predicates, `default_model` and system-prompt override rules.
//! References into the catalog (models, endpoints, pricing, governance) are
//! checked when the catalog is resolved and fail with
//! [`CatalogError`](crate::services::catalog::CatalogError).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::RouteId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayProfileError {
    #[error("gateway route id '{id}' is declared more than once")]
    DuplicateRouteId { id: RouteId },

    #[error("a gateway route declares an empty `id`; every route has a declared, stable id")]
    EmptyRouteId,

    #[error("gateway route '{route}' matches nothing: it declares neither `models` nor `alias`")]
    RouteMatchesNothing { route: RouteId },

    #[error(
        "gateway route '{route}' gives deployment '{endpoint}' `weight: 0`; a weight is at least \
         1 (remove the deployment to stop sending to it)"
    )]
    RouteDeploymentWeightZero { route: RouteId, endpoint: String },

    #[error(
        "gateway `{field}` is '{value}': the `[1m]` context marker is a client presentation \
         derived from the model's `limits.context_window`, never written in config"
    )]
    ContextVariantInConfig { field: &'static str, value: String },

    #[error("system_prompt override with action 'replace' must set a 'prompt'")]
    OverrideReplaceMissingPrompt,

    #[error("system_prompt override with action 'strip' must not set a 'prompt'")]
    OverrideStripWithPrompt,

    #[error("route `when.scope` maps a dimension to an empty value")]
    RouteScopeValueEmpty,

    #[error("route `when.min_tools` must be at least 1 (0 matches every request)")]
    RouteMatchZeroMinTools,

    #[error("route `when` sets `requires_tools: false` but also a positive `min_tools`")]
    RouteMatchContradictoryTools,

    #[error("route `when.tool_types_any` entry `{0}` is not a server tool type prefix")]
    RouteMatchBadToolType(String),

    #[error(
        "gateway `bridge_instance.{field}` is '{value}': it must be a path on this gateway \
         starting with '/', with no scheme or host"
    )]
    BridgeInstancePathNotRelative { field: &'static str, value: String },

    #[error(
        "gateway `bridge_releases.{field}` is '{version}': build metadata ('+...') is ignored by \
         semver precedence, so a window bound must not carry it"
    )]
    BridgeVersionBuildMetadata {
        field: &'static str,
        version: semver::Version,
    },

    #[error("gateway `bridge_releases.min_version` {min} is above `max_version` {max}")]
    BridgeVersionWindowInverted {
        min: semver::Version,
        max: semver::Version,
    },

    #[error("gateway `bridge_releases.pinned_version` '{value}' is not a semver version")]
    BridgePinnedVersionInvalid {
        value: String,
        #[source]
        source: semver::Error,
    },

    #[error(
        "gateway `bridge_releases.pinned_version` {pinned} lies outside the window \
         [`min_version`, `max_version`]"
    )]
    BridgePinnedVersionOutsideWindow { pinned: semver::Version },
}

pub type GatewayResult<T> = Result<T, GatewayProfileError>;
