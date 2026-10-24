//! Failure modes of resolving the catalog ([`CatalogError`]), of reading a
//! catalog document ([`CatalogDocumentError`]), and of resolving one request
//! against a resolved catalog ([`ResolveError`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{CredentialId, EndpointId, ModelId, ProviderId, RouteId};
use systemprompt_models::net::OutboundUrlError;
use thiserror::Error;

use crate::services::gateway::GatewayProfileError;

pub const MIGRATE_HINT: &str = "run `systemprompt admin config migrate` to rewrite it";

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error(transparent)]
    Gateway(#[from] GatewayProfileError),

    #[error("credential '{credential}' is `kind: platform` but names no `secret`")]
    PlatformCredentialWithoutSecret { credential: CredentialId },

    #[error(
        "credential '{credential}' is `kind: personal` but names secret '{secret}'; each member \
         stores their own key, so a personal credential has no deployment secret"
    )]
    PersonalCredentialWithSecret {
        credential: CredentialId,
        secret: String,
    },

    #[error(
        "provider '{provider}' names credential '{credential}', which credentials.yaml does not declare"
    )]
    UnknownCredential {
        provider: ProviderId,
        credential: CredentialId,
    },

    #[error(
        "personal credential '{credential}' is used by providers '{first}' and '{second}'; one \
         personal subscription is one vendor account, so declare its endpoints under one provider"
    )]
    PersonalCredentialShared {
        credential: CredentialId,
        first: ProviderId,
        second: ProviderId,
    },

    #[error("provider '{provider}' declares no endpoints")]
    ProviderWithoutEndpoints { provider: ProviderId },

    #[error(
        "provider '{provider}' sets reserved header '{header}'; the credential, content framing \
         and protocol version are sent by the gateway"
    )]
    ReservedHeader {
        provider: ProviderId,
        header: String,
    },

    #[error("endpoint '{provider}/{endpoint}' has an empty url")]
    EmptyEndpointUrl {
        provider: ProviderId,
        endpoint: EndpointId,
    },

    #[error("endpoint '{provider}/{endpoint}' url '{url}' is not permitted: {source}")]
    BlockedEndpointUrl {
        provider: ProviderId,
        endpoint: EndpointId,
        url: String,
        #[source]
        source: OutboundUrlError,
    },

    #[error(
        "endpoint '{provider}/{endpoint}' url '{url}' names a Google Cloud project literally; the \
         project is derived from the service account in the credential's secret — write \
         `projects/{{project}}`"
    )]
    LiteralProjectInUrl {
        provider: ProviderId,
        endpoint: EndpointId,
        url: String,
    },

    #[error("endpoint '{provider}/{endpoint}' price_multiplier must be a positive number")]
    InvalidPriceMultiplier {
        provider: ProviderId,
        endpoint: EndpointId,
    },

    #[error(
        "endpoint '{provider}/{endpoint}' accepted_betas '{value}' is neither a list nor \
         `inherit:<endpoint>`"
    )]
    MalformedBetaInherit {
        provider: ProviderId,
        endpoint: EndpointId,
        value: String,
    },

    #[error(
        "endpoint '{provider}/{endpoint}' inherits accepted_betas from '{target}', which is not \
         an endpoint of the same provider"
    )]
    UnknownBetaInherit {
        provider: ProviderId,
        endpoint: EndpointId,
        target: EndpointId,
    },

    #[error("endpoint '{provider}/{endpoint}' accepted_betas inherit chain is cyclic")]
    CyclicBetaInherit {
        provider: ProviderId,
        endpoint: EndpointId,
    },

    #[error("models.yaml declares a model with an empty id")]
    EmptyModelId,

    #[error(
        "model id '{model}' carries the `[1m]` context marker; it is derived from \
         `limits.context_window`, never written in config"
    )]
    ContextVariantModelId { model: ModelId },

    #[error("model '{model}' has no deployment in deployments.yaml")]
    ModelWithoutDeployment { model: ModelId },

    #[error("deployments.yaml deploys '{model}', which models.yaml does not declare")]
    DeploymentForUnknownModel { model: ModelId },

    #[error("deployment of '{model}' names endpoint '{endpoint}', which no provider declares")]
    UnknownEndpoint { model: ModelId, endpoint: String },

    #[error("deployment of '{model}' lists endpoint '{endpoint}' more than once")]
    DuplicateDeployment { model: ModelId, endpoint: String },

    #[error(
        "'{model}' is deployed on '{endpoint}', which has no usable pricing (input and output \
         per million must both be non-zero, or per_image_cents set)"
    )]
    UnpricedDeployment { model: ModelId, endpoint: String },

    #[error(
        "'{model}' on '{endpoint}' declares no `cache_read_per_million`; `input_tokens` excludes \
         cached prompt tokens, so an absent rate bills the cached slice at zero -- set the \
         published rate, or an explicit 0.0 if it bills none"
    )]
    CacheRateUndeclared { model: ModelId, endpoint: String },

    #[error(
        "provider '{provider}' sets google_search_enabled but declares no `gemini`-wire endpoint"
    )]
    GoogleSearchWithoutGemini { provider: ProviderId },

    #[error("ai.default_image_model '{model}' is not a deployed catalog model")]
    UnknownDefaultImageModel { model: ModelId },

    #[error("ai.default_image_model '{model}' does not have the image_generation capability")]
    DefaultImageModelNotImageCapable { model: ModelId },

    #[error("gateway default_model '{model}' is not a catalog model")]
    UnknownDefaultModel { model: ModelId },

    #[error("gateway default_model '{model}' has no deployment on an enabled provider")]
    DefaultModelNotServable { model: ModelId },

    #[error("gateway route '{route}' aliases to '{model}', which is not a catalog model")]
    UnknownAliasTarget { route: RouteId, model: ModelId },

    #[error(
        "gateway route '{route}' aliases '{requested}', which is already a catalog model id or \
         another route's alias"
    )]
    AliasShadowsModel { route: RouteId, requested: ModelId },

    #[error("gateway route '{route}' models glob matches no catalog model")]
    RouteMatchesNoModel { route: RouteId },

    #[error(
        "gateway route '{route}' pins '{endpoint}' for '{model}', which is not one of that \
         model's deployments; a route narrows a model's chain, it never adds to it"
    )]
    RouteDeploymentNotDeployed {
        route: RouteId,
        model: ModelId,
        endpoint: String,
    },

    #[error("gateway route '{route}' context fallback '{model}' is not a catalog model")]
    UnknownContextFallback { route: RouteId, model: ModelId },

    #[error(
        "gateway route '{route}' requires [{requirements}] but can dispatch '{model}' on \
         '{endpoint}', whose governance does not declare them"
    )]
    RouteGovernanceUnsatisfied {
        route: RouteId,
        model: ModelId,
        endpoint: String,
        requirements: String,
    },
}

#[derive(Debug, Error)]
pub enum CatalogDocumentError {
    #[error("{origin}: {source}")]
    Parse {
        origin: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("{origin}: {what} is the pre-0.73 catalog shape; {MIGRATE_HINT}")]
    LegacyShape { origin: String, what: &'static str },

    #[error("{section} id '{id}' is declared in more than one catalog file")]
    DuplicateId { section: &'static str, id: String },

    #[error("more than one catalog file declares `gateway:`")]
    DuplicateGateway,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResolveError {
    #[error("model '{requested}' is not in the catalog")]
    UnknownModel { requested: String },

    #[error("model '{model}' has no deployment on an enabled provider")]
    NoEnabledDeployment { model: ModelId },
}

pub type CatalogResult<T> = Result<T, CatalogError>;
