//! Failure modes of
//! [`ProviderRegistry::validate`](super::ProviderRegistry::validate): duplicate
//! provider names, empty or SSRF-blocked endpoints, and duplicate or
//! empty model ids/aliases. Connectivity is the registry's authority, so these
//! are the only errors emitted while checking it.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderRegistryError {
    #[error("provider registry declares provider name '{name}' more than once")]
    DuplicateProvider { name: String },

    #[error("provider registry entry '{name}' has an empty endpoint")]
    EmptyEndpoint { name: String },

    #[error(
        "provider registry entry '{provider}' endpoint '{endpoint}' is not permitted: {reason}"
    )]
    BlockedEndpoint {
        provider: String,
        endpoint: String,
        reason: String,
    },

    #[error("provider registry model id or alias '{id}' is declared more than once")]
    DuplicateModel { id: String },

    #[error("provider registry model '{id}' has an empty id")]
    EmptyModelId { id: String },
}

pub type ProviderRegistryResult<T> = Result<T, ProviderRegistryError>;
