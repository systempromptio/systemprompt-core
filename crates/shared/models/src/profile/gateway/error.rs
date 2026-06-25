//! Failure modes emitted while validating the gateway's references into the
//! provider registry: duplicate route ids, and a route or `default_provider`
//! naming a provider absent from `profile.providers`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayProfileError {
    #[error("gateway route id '{id}' is declared more than once")]
    DuplicateRouteId { id: String },

    #[error("gateway route '{route}' provider '{provider}' is not declared in profile.providers")]
    RouteProviderNotInRegistry { route: String, provider: String },

    #[error("gateway default_provider '{provider}' is not declared in profile.providers")]
    DefaultProviderNotInRegistry { provider: String },

    #[error("system_prompt override with action 'replace' must set a 'prompt'")]
    OverrideReplaceMissingPrompt,

    #[error("system_prompt override with action 'strip' must not set a 'prompt'")]
    OverrideStripWithPrompt,

    #[error("system_prompt override provider '{provider}' is not declared in profile.providers")]
    OverrideProviderNotInRegistry { provider: String },

    #[error("route `when.min_tools` must be at least 1 (0 matches every request)")]
    RouteMatchZeroMinTools,

    #[error("route `when` sets `requires_tools: false` but also a positive `min_tools`")]
    RouteMatchContradictoryTools,
}

pub type GatewayResult<T> = Result<T, GatewayProfileError>;
