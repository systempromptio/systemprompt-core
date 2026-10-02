//! Failure modes of
//! [`ProviderRegistry::validate`](super::ProviderRegistry::validate): duplicate
//! provider names, empty or SSRF-blocked endpoints, reserved `extra_headers`
//! names, and duplicate or empty model ids/aliases. Connectivity is the
//! registry's authority, so these are the only errors emitted while checking
//! it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ModelId, ProviderId};
use thiserror::Error;

use crate::net::OutboundUrlError;

#[derive(Debug, Error)]
pub enum ProviderRegistryError {
    #[error("provider registry declares provider name '{name}' more than once")]
    DuplicateProvider { name: String },

    #[error("provider registry entry '{name}' has an empty endpoint")]
    EmptyEndpoint { name: String },

    #[error(
        "provider registry entry '{provider}' endpoint '{endpoint}' is not permitted: {source}"
    )]
    BlockedEndpoint {
        provider: String,
        endpoint: String,
        #[source]
        source: OutboundUrlError,
    },

    #[error(
        "provider registry entry '{provider}' endpoint '{endpoint}' names a Google Cloud project literally; \
         the project is derived from the service account in its secret — write `projects/{{project}}`"
    )]
    LiteralProjectInEndpoint { provider: String, endpoint: String },

    #[error(
        "provider registry entry '{provider}' sets reserved header '{header}' in extra_headers; \
         the credential, content framing and protocol version are sent by the gateway"
    )]
    ReservedExtraHeader { provider: String, header: String },

    #[error("provider registry model id or alias '{id}' is declared more than once")]
    DuplicateModel { id: ModelId },

    #[error("provider registry entry '{provider}' declares a model with an empty id")]
    EmptyModelId { provider: ProviderId },

    #[error("embedded default provider catalog failed to parse: {0}")]
    InvalidDefaultCatalog(#[source] serde_yaml::Error),

    #[error("embedded Vertex rate card failed to parse: {0}")]
    VertexRateCardParse(#[source] serde_yaml::Error),

    #[error("embedded Vertex rate card entry '{entry}' is invalid: {defect}")]
    InvalidVertexRateCard {
        entry: ModelId,
        defect: VertexRateCardDefect,
    },
}

/// The rule an embedded Vertex rate card entry breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VertexRateCardDefect {
    #[error("`docs` must be the official documentation URL")]
    DocsNotOfficial,

    #[error("`retires_on` is not after `released`")]
    RetiresBeforeRelease,

    #[error("`price_until` is not after `released`")]
    PriceUntilBeforeRelease,
}

pub type ProviderRegistryResult<T> = Result<T, ProviderRegistryError>;
