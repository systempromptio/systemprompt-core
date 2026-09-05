//! Failure modes emitted while validating the gateway's references into the
//! provider registry: duplicate route ids, and a route or `default_provider`
//! naming a provider absent from the services provider registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayProfileError {
    #[error("gateway route id '{id}' is declared more than once")]
    DuplicateRouteId { id: String },

    #[error("gateway route '{route}' provider '{provider}' is not declared in services providers")]
    RouteProviderNotInRegistry { route: String, provider: String },

    #[error("gateway default_provider '{provider}' is not declared in services providers")]
    DefaultProviderNotInRegistry { provider: String },

    #[error("system_prompt override with action 'replace' must set a 'prompt'")]
    OverrideReplaceMissingPrompt,

    #[error("system_prompt override with action 'strip' must not set a 'prompt'")]
    OverrideStripWithPrompt,

    #[error("system_prompt override provider '{provider}' is not declared in services providers")]
    OverrideProviderNotInRegistry { provider: String },

    #[error("route `when.min_tools` must be at least 1 (0 matches every request)")]
    RouteMatchZeroMinTools,

    #[error("route `when` sets `requires_tools: false` but also a positive `min_tools`")]
    RouteMatchContradictoryTools,

    #[error(
        "gateway route '{route}' (model_pattern '{pattern}') reaches no priced model: provider \
         '{provider}' declares no model matching the pattern and the route sets no `pricing:` \
         override, so every request it dispatches would be billed at zero"
    )]
    RouteReachesNoPricedModel {
        route: String,
        pattern: String,
        provider: String,
    },

    #[error(
        "gateway route '{route}' requires [{requirements}] but can dispatch model '{model}', \
         whose provider/model governance does not declare them — annotate the provider or model \
         with `governance:` or drop the route requirement"
    )]
    RouteGovernanceUnsatisfied {
        route: String,
        model: String,
        requirements: String,
    },

    #[error(
        "gateway route '{route}' can dispatch model '{model}', which has no usable `pricing:` \
         (input_per_million and output_per_million must both be non-zero, or per_image_cents set)"
    )]
    RouteModelUnpriced { route: String, model: String },

    #[error(
        "gateway route '{route}' can dispatch provider '{provider}' model '{model}', \
         which declares no `cache_read_per_million`; every wire we speak can report cached \
         prompt tokens, and `input_tokens` excludes them, so an absent rate bills the cached \
         slice at zero -- set the provider's published rate, or an explicit 0.0 if it bills \
         none"
    )]
    RouteModelCacheRateUndeclared {
        route: String,
        provider: String,
        model: String,
    },
}

pub type GatewayResult<T> = Result<T, GatewayProfileError>;
